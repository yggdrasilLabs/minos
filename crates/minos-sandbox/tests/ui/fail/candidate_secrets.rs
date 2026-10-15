// Invariant: only a Judge spec can take judge data, so a Candidate spec has no `secrets` method (SBX-01, SBX-02).
use minos_sandbox::{Candidate, JudgeData, SandboxSpec};

fn probe(spec: SandboxSpec<Candidate>, data: JudgeData) -> SandboxSpec<Candidate> {
    spec.secrets(data)
}

fn main() {
    let _probe: fn(SandboxSpec<Candidate>, JudgeData) -> SandboxSpec<Candidate> = probe;
}
