//! `minos host doctor` and `minos host setup`.

use std::process::ExitCode;

use anyhow::bail;

/// Runs the launch preflight and explains every refusal.
pub fn doctor() -> anyhow::Result<ExitCode> {
    bail!("minos host doctor is not built yet")
}

/// Signs and verifies the `minos-vmm` helper.
pub fn setup() -> anyhow::Result<ExitCode> {
    bail!("minos host setup is not built yet")
}
