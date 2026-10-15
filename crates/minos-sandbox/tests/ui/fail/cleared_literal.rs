// Invariant: `preflight` is the only constructor of Cleared, so no other crate can mint a clearance for a plan
// that never passed preflight (SBX-03).
use minos_sandbox::{Cleared, Judge, LaunchPlan};

fn probe(plan: LaunchPlan<Judge>) -> Cleared<Judge> {
    Cleared { plan }
}

fn main() {
    let _probe: fn(LaunchPlan<Judge>) -> Cleared<Judge> = probe;
}
