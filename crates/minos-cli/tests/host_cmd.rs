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

/// `minos host doctor` from a directory holding only a copy of `minos`: the helper and the guest are refused,
/// every path the locator checked is named, and the exit is 1.
#[test]
fn doctor_without_binaries_refuses_and_names_every_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bin = std::fs::canonicalize(dir.path())?.join("bin");
    std::fs::create_dir(&bin)?;
    let minos = bin.join("minos");
    std::fs::copy(env!("CARGO_BIN_EXE_minos"), &minos)?;

    let output = Command::new(&minos).args(["host", "doctor"]).output()?;

    let report = String::from_utf8(output.stdout)?;
    assert_eq!(output.status.code(), Some(1), "{report}");
    for line in [
        "refused  helper present  the minos-vmm helper was not found",
        "refused  guest binary  no static aarch64 Linux minos-guest was found",
        "dedicated user: missing (deferred, reduced hardening)",
        "builder: ",
    ] {
        assert!(report.contains(line), "{line:?} missing in:\n{report}");
    }
    for path in [bin.join("minos-vmm"), bin.join("minos-guest")] {
        let checked = format!("  checked: {}", path.display());
        assert!(
            report.contains(&checked),
            "{checked:?} missing in:\n{report}"
        );
    }
    Ok(())
}

/// `minos host setup` with no helper next to `minos` refuses with the build fix and exits 1, signing nothing.
#[test]
fn setup_without_helper_refuses() -> TestResult {
    let dir = tempfile::tempdir()?;
    let minos = std::fs::canonicalize(dir.path())?.join("minos");
    std::fs::copy(env!("CARGO_BIN_EXE_minos"), &minos)?;

    let output = Command::new(&minos).args(["host", "setup"]).output()?;

    let report = String::from_utf8(output.stdout)?;
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("build it with cargo build -p minos-vmm"),
        "{report}"
    );
    Ok(())
}

/// The real host: every check passes.
#[test]
#[ignore = "needs target/debug/minos-vmm built and signed (cargo build -p minos-vmm, then target/debug/minos host setup), the musl guest (cargo build -p minos-guest --release --target aarch64-unknown-linux-musl), and libkrunfw 5.6.2"]
fn doctor_runs_on_this_mac() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_minos"))
        .args(["host", "doctor"])
        .output()?;
    let report = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "{report}");
    assert!(
        !report.lines().any(|line| line.starts_with("refused")),
        "{report}"
    );
    Ok(())
}
