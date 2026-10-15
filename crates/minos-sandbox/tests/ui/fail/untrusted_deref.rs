// Invariant: Untrusted has no Deref, so its contents cannot be reached with `*` (SBX-04).
use minos_sandbox::{GuestOutput, Judge, Untrusted};

fn probe(output: Untrusted<GuestOutput<Judge>>) {
    let _inner = &*output;
}

fn main() {
    let _probe: fn(Untrusted<GuestOutput<Judge>>) = probe;
}
