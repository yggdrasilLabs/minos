// Invariant: Untrusted's field is private, so its contents cannot be reached with `.0` (SBX-04).
use minos_sandbox::{GuestOutput, Judge, Untrusted};

fn probe(output: Untrusted<GuestOutput<Judge>>) -> GuestOutput<Judge> {
    output.0
}

fn main() {
    let _probe: fn(Untrusted<GuestOutput<Judge>>) -> GuestOutput<Judge> = probe;
}
