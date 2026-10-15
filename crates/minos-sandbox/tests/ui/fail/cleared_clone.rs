// Invariant: Cleared is not Clone, so one clearance authorizes one launch (SBX-03).
use minos_sandbox::{Cleared, Judge};

fn probe(ok: Cleared<Judge>) -> (Cleared<Judge>, Cleared<Judge>) {
    let copy = ok.clone();
    (ok, copy)
}

fn main() {
    let _probe: fn(Cleared<Judge>) -> (Cleared<Judge>, Cleared<Judge>) = probe;
}
