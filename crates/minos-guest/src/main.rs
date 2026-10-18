//! Guest binary that runs as PID 1 inside every VM.
//!
//! On Linux it dispatches on its arguments and pid: with `--minos-stage` as the first argument it is the stage
//! trampoline, as PID 1 it runs the job, and otherwise it explains itself and fails. On any other OS it only
//! explains that it must run as `/init` inside a Linux VM, so the host gate still compiles the binary.

#[cfg(target_os = "linux")]
mod linux;

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    entry()
}

#[cfg(target_os = "linux")]
fn entry() -> ExitCode {
    use minos_guest::stage::TRAMPOLINE_FLAG;

    if std::env::args_os()
        .nth(1)
        .is_some_and(|flag| flag == TRAMPOLINE_FLAG)
    {
        return linux::trampoline();
    }
    if rustix::process::getpid().is_init() {
        linux::pid1()
    }
    usage()
}

#[cfg(not(target_os = "linux"))]
fn entry() -> ExitCode {
    usage()
}

fn usage() -> ExitCode {
    // A failed stderr write has nowhere else to go; it must not panic the way eprintln! would.
    drop(writeln!(
        std::io::stderr(),
        "minos-guest: runs only as /init inside a Linux VM"
    ));
    ExitCode::FAILURE
}
