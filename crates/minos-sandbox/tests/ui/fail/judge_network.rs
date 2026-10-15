// Invariant: no role gets a network device, so even a Judge spec has no `network` method (SBX-01).
use minos_sandbox::{Judge, SandboxSpec};

fn probe(spec: SandboxSpec<Judge>) -> SandboxSpec<Judge> {
    spec.network()
}

fn main() {
    let _probe: fn(SandboxSpec<Judge>) -> SandboxSpec<Judge> = probe;
}
