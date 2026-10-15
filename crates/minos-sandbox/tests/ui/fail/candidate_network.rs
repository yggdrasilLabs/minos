// Invariant: no role gets a network device, so a Candidate spec has no `network` method (SBX-01).
use minos_sandbox::{Candidate, SandboxSpec};

fn probe(spec: SandboxSpec<Candidate>) -> SandboxSpec<Candidate> {
    spec.network()
}

fn main() {
    let _probe: fn(SandboxSpec<Candidate>) -> SandboxSpec<Candidate> = probe;
}
