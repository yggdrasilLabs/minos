//! The `minos` command-line interface.
//!
//! The command tree is built with clap's builder API; the derive feature stays off, because `clap_derive`'s
//! generated `#[allow(clippy::restriction)]` is E0453 under the workspace's `forbid` lints (01-GATE-EVIDENCE.md
//! F-01). Commands so far:
//!
//! - `minos host doctor`: run the launch preflight and explain every refusal (CLI-03, KRUN-06).
//! - `minos host setup`: sign the `minos-vmm` helper and verify its entitlements (KRUN-08).
//!
//! Plan 05-16 adds `minos env build`, and Phase 8 adds the rest of the tree CLAUDE.md lists. `anyhow` appears
//! only here, at the binary boundary (rules/err-anyhow-app); the library half is in `minos_cli`.

mod host;

use std::process::ExitCode;

use anyhow::anyhow;
use clap::Command;
use minos_core::text::SafeText;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            // Errors can carry paths and host-tool text, so they are printed through the renderer.
            eprintln!("minos: error: {}", SafeText::new(&format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}

/// The `minos` command tree.
fn cli() -> Command {
    Command::new("minos")
        .about("A secure lab for autonomous experiment loops")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("host")
                .about("Check and prepare this machine for running sandboxes")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("doctor")
                        .about("run the launch preflight and explain every refusal"),
                )
                .subcommand(Command::new("setup").about("sign and verify the minos-vmm helper")),
        )
}

/// Parses the command line and runs the chosen command.
fn run() -> anyhow::Result<ExitCode> {
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("host", host)) => match host.subcommand() {
            Some(("doctor", _)) => host::doctor(),
            Some(("setup", _)) => host::setup(),
            _ => Err(anyhow!("unknown host command")),
        },
        _ => Err(anyhow!("unknown command")),
    }
}

#[cfg(test)]
mod tests {
    use super::cli;

    #[test]
    fn command_tree_is_well_formed() {
        cli().debug_assert();
    }
}
