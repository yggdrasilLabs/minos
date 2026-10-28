//! `minos run --generations N [--run <prefix>]`: start a new run, or with `--run` resume one, and drive it to the
//! absolute generation N, printing one fixed-width progress line per recorded trial, decision, retry, and generation
//! cost (CLI-06; 08-CONTEXT D-14; 08-RESEARCH Open Question 1).
//!
//! # The progress format
//!
//! Plain text, fixed width, no color (D-14). The formatters below are pure and are the only place a progress line
//! is laid out; `tests/progress_golden.rs` pins them byte for byte. A trial line has six columns separated by one
//! space:
//!
//! | Column | Width | Content |
//! |---|---|---|
//! | generation | `g` and at least two digits | `g03`; a generation past 99 widens on its own |
//! | experiment | 12, left-aligned | the experiment ID's first 12 hex characters |
//! | tier | 11, left-aligned | `sweep`, `single_shot`, or `baseline` in generation 0 |
//! | outcome | 26, left-aligned | `scored`, `rejected:<reason>`, `crashed:<stage>`, `timed_out:<stage>`; 26 fits the widest, `rejected:held_out_mismatch` |
//! | delta | 7, right-aligned | the delta as a signed percent of the incumbent's objective (`+6.2%`), or `n/a` |
//! | elapsed | 7, right-aligned | the candidate VM's plus the judge VM's host-measured time (`1m42s`), or `n/a` |
//!
//! For example `g03 3f9a2c1d7e4b single_shot scored                       +6.2%   1m42s`. The other lines are
//! `g03 proposed single_shot admitted 4 dropped 1`, `g03 selected improved 3f9a2c1d7e4b` (or `kept <incumbent>`,
//! or `no_valid_candidates`), `retrying after an infrastructure failure (1 of 3)`, `g03 stopped user_stop`, and
//! `g03 approx. cost: $0.42 (in 1500 tokens, out 456 tokens)`, the cost line for the run so far, written only by
//! [`CostLine`] (COST-02). No line ends in a space, and every printed line still passes through
//! [`crate::render::write_line`].

use minos_core::money::CostLine;

/// A trial's progress line: generation, experiment prefix, tier, outcome word, delta percent, and elapsed time, laid
/// out as the [module docs](self) give.
#[must_use]
pub fn trial_line(
    generation: u32,
    exp: &str,
    tier: &str,
    outcome: &str,
    percent: Option<f64>,
    elapsed_ms: Option<u64>,
) -> String {
    let percent = crate::render::percent_text(percent);
    let elapsed = crate::render::elapsed_text(elapsed_ms);
    format!("g{generation:02} {exp:<12} {tier:<11} {outcome:<26} {percent:>7} {elapsed:>7}")
}

/// A propose step's line: `g03 proposed single_shot admitted 4 dropped 1`.
#[must_use]
pub fn proposed_line(generation: u32, tier: &str, admitted: u32, dropped: u32) -> String {
    format!("g{generation:02} proposed {tier} admitted {admitted} dropped {dropped}")
}

/// A decision's line: `g03 selected improved 3f9a2c1d7e4b`, or without an experiment
/// `g03 selected no_valid_candidates`.
#[must_use]
pub fn selected_line(generation: u32, decision: &str, exp: Option<&str>) -> String {
    match exp {
        Some(exp) => format!("g{generation:02} selected {decision} {exp}"),
        None => format!("g{generation:02} selected {decision}"),
    }
}

/// An infrastructure retry's line: `retrying after an infrastructure failure (1 of 3)`.
#[must_use]
pub fn retrying_line(streak: u32, max: u32) -> String {
    format!("retrying after an infrastructure failure ({streak} of {max})")
}

/// A stop's line: `g03 stopped user_stop`.
#[must_use]
pub fn stopped_line(generation: u32, reason: &str) -> String {
    format!("g{generation:02} stopped {reason}")
}

/// A generation's cost line for the run so far: `g03 ` and then the [`CostLine`] text.
#[must_use]
pub fn cost_text(generation: u32, cost: &CostLine) -> String {
    format!("g{generation:02} {cost}")
}
