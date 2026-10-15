// Invariant: Untrusted output has no Debug, so hostile bytes cannot reach a log through `{:?}` (SBX-04).
use minos_sandbox::{GuestOutput, Judge, Untrusted};

fn probe(output: Untrusted<GuestOutput<Judge>>) -> String {
    format!("{output:?}")
}

fn main() {
    let _probe: fn(Untrusted<GuestOutput<Judge>>) -> String = probe;
}
