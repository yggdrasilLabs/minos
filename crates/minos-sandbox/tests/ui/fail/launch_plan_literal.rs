// Invariant: `seal` is the only constructor of a LaunchPlan, so no other crate can build one with a struct
// literal (SBX-03).
use std::marker::PhantomData;

use minos_core::id::ImageDigest;
use minos_core::job::JobBudget;
use minos_core::manifest::Arch;
use minos_sandbox::{Candidate, InputDisk, LaunchKey, LaunchPlan, Machine};

fn probe(
    key: LaunchKey,
    root: ImageDigest,
    input: InputDisk,
    budget: JobBudget,
    machine: Machine,
    arch: Arch,
) -> LaunchPlan<Candidate> {
    LaunchPlan {
        key,
        root,
        input,
        budget,
        machine,
        arch,
        max_duration_ms: 1,
        judge_data: None,
        role: PhantomData,
    }
}

fn main() {
    let _probe: fn(
        LaunchKey,
        ImageDigest,
        InputDisk,
        JobBudget,
        Machine,
        Arch,
    ) -> LaunchPlan<Candidate> = probe;
}
