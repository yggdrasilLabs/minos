//! The `minos` binary's command tree, run as a process.

use std::error::Error;
use std::process::Command;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn cli_help_lists_host_commands() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_minos"))
        .args(["host", "--help"])
        .output()?;
    assert!(output.status.success(), "minos host --help failed");
    let help = String::from_utf8(output.stdout)?;
    assert!(help.contains("doctor"), "no doctor in: {help}");
    assert!(help.contains("setup"), "no setup in: {help}");
    Ok(())
}
