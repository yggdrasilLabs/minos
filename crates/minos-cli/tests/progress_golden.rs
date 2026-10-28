//! The pinned progress format of `minos run` (08-CONTEXT D-14): fixed-width plain text, no color, one line per
//! recorded trial, proposal step, decision, retry, stop, and generation cost.
//!
//! The literal below is the golden. It covers a scored trial with a positive and a negative percent, a baseline
//! trial, each outcome kind (`rejected:gate_failed`, `crashed:build`, `timed_out:run`, and the widest word,
//! `rejected:held_out_mismatch`), a three-digit generation, the `proposed` line, the three `selected` forms, the
//! `retrying` and `stopped` lines, and a cost line through `CostLine`. Every line must also be stable under
//! `SafeText` and must not end in a space.

use std::error::Error;

use minos_cli::run::{
    cost_text, proposed_line, retrying_line, selected_line, stopped_line, trial_line,
};
use minos_core::money::{ApproxCost, CostLine};
use minos_core::text::SafeText;
use minos_core::usage::Usage;

type TestResult = Result<(), Box<dyn Error>>;

/// The expected progress text, byte for byte.
const GOLDEN: &str = "\
g03 3f9a2c1d7e4b single_shot scored                       +6.2%   1m42s
g03 7c0d5e6f8a9b single_shot scored                       -3.5%   1m05s
g00 0a1b2c3d4e5f baseline    scored                         n/a   0m41s
g12 5e5e5e5e5e5e sweep       rejected:gate_failed           n/a     n/a
g04 1111aaaa2222 sweep       crashed:build                  n/a   0m05s
g04 3333bbbb4444 sweep       timed_out:run                  n/a   5m00s
g05 9999cccc8888 single_shot rejected:held_out_mismatch     n/a   1m01s
g123 abcdefabcdef sweep       scored                     +100.0%  60m00s
g03 proposed single_shot admitted 4 dropped 1
g03 selected improved 3f9a2c1d7e4b
g04 selected kept 3f9a2c1d7e4b
g05 selected no_valid_candidates
retrying after an infrastructure failure (1 of 3)
g03 stopped user_stop
g03 approx. cost: $0.42 (in 1500 tokens, out 456 tokens)";

/// Every formatter with fixed inputs, in the golden's order.
fn lines() -> Vec<String> {
    let usage = Usage::tokens(1500, 456, 0, 0);
    let cost = CostLine::new(ApproxCost::from_micro(420_000), &usage);
    vec![
        trial_line(
            3,
            "3f9a2c1d7e4b",
            "single_shot",
            "scored",
            Some(6.2),
            Some(102_000),
        ),
        trial_line(
            3,
            "7c0d5e6f8a9b",
            "single_shot",
            "scored",
            Some(-3.5),
            Some(65_000),
        ),
        trial_line(0, "0a1b2c3d4e5f", "baseline", "scored", None, Some(41_000)),
        trial_line(12, "5e5e5e5e5e5e", "sweep", "rejected:gate_failed", None, None),
        trial_line(4, "1111aaaa2222", "sweep", "crashed:build", None, Some(5_000)),
        trial_line(4, "3333bbbb4444", "sweep", "timed_out:run", None, Some(300_000)),
        trial_line(
            5,
            "9999cccc8888",
            "single_shot",
            "rejected:held_out_mismatch",
            None,
            Some(61_000),
        ),
        trial_line(
            123,
            "abcdefabcdef",
            "sweep",
            "scored",
            Some(100.0),
            Some(3_600_000),
        ),
        proposed_line(3, "single_shot", 4, 1),
        selected_line(3, "improved", Some("3f9a2c1d7e4b")),
        selected_line(4, "kept", Some("3f9a2c1d7e4b")),
        selected_line(5, "no_valid_candidates", None),
        retrying_line(1, 3),
        stopped_line(3, "user_stop"),
        cost_text(3, &cost),
    ]
}

#[test]
fn the_progress_lines_match_the_golden_text() {
    assert_eq!(lines().join("\n"), GOLDEN);
}

#[test]
fn no_progress_line_ends_in_a_space_or_holds_a_newline() {
    for line in lines() {
        assert!(!line.ends_with(' '), "{line:?}");
        assert!(!line.contains('\n'), "{line:?}");
    }
}

#[test]
fn every_progress_line_is_stable_under_safe_text() {
    for line in lines() {
        assert_eq!(SafeText::new(&line).as_str(), line);
    }
}

#[test]
fn the_documented_examples_hold() -> TestResult {
    let baseline = trial_line(0, "0a1b2c3d4e5f", "baseline", "scored", None, Some(41_000));
    let tier = baseline.get(17..28).ok_or("the baseline line is short")?;
    assert_eq!(tier, "baseline   ");
    let gate = trial_line(12, "5e5e5e5e5e5e", "sweep", "rejected:gate_failed", None, None);
    assert!(gate.starts_with("g12 "), "{gate}");
    assert!(gate.ends_with("n/a     n/a"), "{gate}");
    Ok(())
}
