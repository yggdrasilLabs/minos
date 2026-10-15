// Invariant: only judge output can become a scorecard, so candidate output has no `into_scorecard` (D-14,
// GUEST-04).
use minos_core::limits::Limits;
use minos_core::metric::Scorecard;
use minos_sandbox::{Candidate, GuestOutput, ScorecardFault, Untrusted};

fn probe(
    output: Untrusted<GuestOutput<Candidate>>,
    limits: &Limits,
) -> Result<Scorecard, ScorecardFault> {
    output.into_scorecard(limits)
}

fn main() {
    let _probe: fn(
        Untrusted<GuestOutput<Candidate>>,
        &Limits,
    ) -> Result<Scorecard, ScorecardFault> = probe;
}
