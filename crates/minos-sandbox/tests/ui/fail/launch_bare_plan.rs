// Invariant: a backend launches only a Cleared plan, never a bare LaunchPlan that skipped preflight (SBX-02,
// SBX-03).
use minos_sandbox::{Backend, Candidate, LaunchError, LaunchPlan};

fn probe<B: Backend>(
    backend: &B,
    plan: LaunchPlan<Candidate>,
) -> Result<B::Running<Candidate>, LaunchError> {
    backend.launch(plan)
}

fn main() {
    // `probe` is generic over the backend, so it cannot be coerced to a function pointer here; the type error
    // above stops compilation before the dead-code lint runs, so no warning joins the stderr.
}
