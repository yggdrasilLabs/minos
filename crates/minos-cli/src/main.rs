//! The `minos` command-line interface.
//!
//! The command tree is built with clap's builder API; the derive feature stays off, because `clap_derive` puts an
//! allow attribute for `clippy::restriction` on its generated impls, which is E0453 under the workspace's `forbid`
//! lints (01-GATE-EVIDENCE.md F-01). Commands so far:
//!
//! - `minos host doctor`: run the launch preflight and explain every refusal (CLI-03, KRUN-06).
//! - `minos host setup`: sign the `minos-vmm` helper and verify its entitlements (KRUN-08).
//! - `minos env build [--update] [--allow-dirty]`: build the project's root image and write `minos.lock`
//!   (KRUN-09, CLI-02; [`minos_cli::env_build`]).
//!
//! Phase 8 adds the rest of the tree CLAUDE.md lists. `anyhow` appears
//! only here, at the binary boundary (rules/err-anyhow-app); the library half is in `minos_cli`.

mod host;

use std::env;
use std::process::ExitCode;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command};
use minos_cli::builder::DockerBuildx;
use minos_cli::env_build::{EnvBuildError, EnvBuildOptions, env_build};
use minos_cli::locate::locate;
use minos_core::text::SafeText;
use minos_sandbox::Refusal;

/// `minos env build`'s long help: what it does, and the uid rule a Dockerfile must be written for (05-CONTEXT
/// D-08; 05-RESEARCH Pitfall 9).
const ENV_BUILD_HELP: &str = "\
Build the project's root image from its Dockerfile and record it in minos.lock.

The build context is the source snapshot minos run uses: judge paths, the hard denylist (.git/, .minos/, .env*, \
minos.lock, keys), and ignored files are left out, and the secret scan must pass. Every FROM line must pin its \
image by digest (name@sha256:<64 hex>). Docker buildx builds the image for linux/arm64; minos then packs it into a \
read-only SquashFS root image, writes its own minos-guest in last as /init, stores it in .minos/blobs, and records \
its digest, the Dockerfile and guest SHA-256, the FROM digests, and the builder version in minos.lock. A build \
whose image digest differs from minos.lock is refused unless --update is passed.

The Dockerfile's USER is ignored: every file in the image is owned by root, and the build and run commands run as \
uid 64000. Install toolchains into world-readable prefixes such as /usr/local, and set CARGO_HOME and RUSTUP_HOME \
there, never under /root.";

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
        .subcommand(
            Command::new("env")
                .about("Build the project's environment")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("build")
                        .about("build the root image from the Dockerfile and write minos.lock")
                        .long_about(ENV_BUILD_HELP)
                        .arg(
                            Arg::new("update")
                                .long("update")
                                .action(ArgAction::SetTrue)
                                .help("accept a root image whose digest differs from minos.lock"),
                        )
                        .arg(
                            Arg::new("allow-dirty")
                                .long("allow-dirty")
                                .action(ArgAction::SetTrue)
                                .help("skip only the clean check, as minos run does"),
                        ),
                ),
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
        Some(("env", env)) => match env.subcommand() {
            Some(("build", build)) => env_build_command(build),
            _ => Err(anyhow!("unknown env command")),
        },
        _ => Err(anyhow!("unknown command")),
    }
}

/// Runs `minos env build` for the project in the current directory, with the guest at its fixed location and
/// Docker buildx from the user's `PATH`.
fn env_build_command(matches: &ArgMatches) -> anyhow::Result<ExitCode> {
    let project = env::current_dir().context("reading the current directory")?;
    let exe = env::current_exe().context("finding the running minos binary")?;
    let located = locate(&exe);
    let guest = located
        .guest()
        .ok_or_else(|| anyhow!("{}", Refusal::GuestMissing))?;
    let builder = DockerBuildx::find()?;
    let options = EnvBuildOptions::new(matches.get_flag("update"), matches.get_flag("allow-dirty"));
    match env_build(&project, guest, &builder, options) {
        Ok(report) => {
            let pack = report.pack();
            println!("root image: sha256 {}", report.image());
            println!(
                "minos.lock: {}",
                if report.lock_written() {
                    "written"
                } else {
                    "unchanged"
                }
            );
            println!(
                "packed: {} files, {} directories, {} symlinks; /init sha256 {}",
                pack.files(),
                pack.dirs(),
                pack.symlinks(),
                pack.init_sha256()
            );
            for image in report.base_images() {
                println!("base image: {}", SafeText::inline(&image.to_string()));
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(EnvBuildError::LockMismatch { locked, built }) => {
            eprintln!("minos: error: the built root image differs from minos.lock");
            eprintln!("  locked: sha256 {locked}");
            eprintln!("  built:  sha256 {built}");
            eprintln!("rerun with --update to accept the new image");
            Ok(ExitCode::from(1))
        }
        Err(error) => Err(error.into()),
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
