//! Guest binary that runs as PID 1 inside every VM.
//!
//! On Linux it runs the `linux` module's PID 1 path. On any other OS it only explains that it must run as
//! `/init` inside a Linux VM, so the host gate still compiles the binary.

#[cfg(target_os = "linux")]
mod linux;

use std::process::ExitCode;

fn main() -> ExitCode {
    entry()
}

#[cfg(target_os = "linux")]
fn entry() -> ExitCode {
    linux::run()
}

#[cfg(not(target_os = "linux"))]
fn entry() -> ExitCode {
    use std::io::Write as _;
    // A failed stderr write has nowhere else to go; it must not panic the way eprintln! would.
    drop(writeln!(
        std::io::stderr(),
        "minos-guest: runs only as /init inside a Linux VM"
    ));
    ExitCode::FAILURE
}
