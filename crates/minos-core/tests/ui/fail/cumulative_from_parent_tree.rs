// Invariant: a `CumulativeDiff` is minted only against the run's `BaseTree`, so a parent `Tree` cannot be passed
// where the base handle is required (07-CONTEXT D-12, 03-REVIEW WR-03).
use std::collections::BTreeSet;

use minos_core::diff::compose::ComposeError;
use minos_core::diff::cumulative::{CumulativeDiff, cumulative};
use minos_core::{Limits, RelPath, Tree};

fn probe(
    parent: &Tree,
    candidate: &Tree,
    writable: &BTreeSet<RelPath>,
    limits: &Limits,
) -> Result<CumulativeDiff, ComposeError> {
    cumulative(parent, candidate, writable, limits)
}

fn main() {
    let _probe: fn(
        &Tree,
        &Tree,
        &BTreeSet<RelPath>,
        &Limits,
    ) -> Result<CumulativeDiff, ComposeError> = probe;
}
