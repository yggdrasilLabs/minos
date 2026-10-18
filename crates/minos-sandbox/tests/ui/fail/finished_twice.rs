// Invariant: Finished::new consumes the clearance, so one Cleared yields at most one Finished (review WR-03;
// SBX-03).
use std::time::Duration;

use minos_sandbox::{Cleared, Ended, Finished, Judge};

fn probe(ok: Cleared<Judge>) -> (Finished<Judge>, Finished<Judge>) {
    let first = Finished::new(ok, Ended::Exited, Vec::new(), Duration::ZERO, Vec::new());
    let second = Finished::new(ok, Ended::Exited, Vec::new(), Duration::ZERO, Vec::new());
    (first, second)
}

fn main() {
    let _probe: fn(Cleared<Judge>) -> (Finished<Judge>, Finished<Judge>) = probe;
}
