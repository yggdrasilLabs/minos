// Invariant: an `Experiment` is built only from a `CumulativeDiff`, so a bare `Diff::parse` result cannot become
// an experiment ID (07-CONTEXT D-12, 03-REVIEW WR-03).
use minos_core::{Diff, Experiment};

fn probe(diff: Diff) -> Experiment {
    Experiment::candidate(diff)
}

fn main() {
    let _probe: fn(Diff) -> Experiment = probe;
}
