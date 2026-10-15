// Invariant: Untrusted output has no Display, so hostile bytes cannot reach a terminal through `{}` (SBX-04).
use minos_sandbox::{GuestOutput, Judge, Untrusted};

fn probe(output: Untrusted<GuestOutput<Judge>>) -> String {
    format!("{output}")
}

fn main() {
    let _probe: fn(Untrusted<GuestOutput<Judge>>) -> String = probe;
}
