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
//! - `minos judge push <dir> [--update]`: register judge data under the manifest's `[judge] version` label in
//!   `minos.lock` (JUDGE-01, CLI-04; [`minos_cli::judge_push`]).
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
use minos_cli::judge_push::{JudgePushOptions, judge_push};
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
uid 64000 with exactly PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin, HOME=/scratch/home, \
TMPDIR=/tmp, and LANG=C, so ENV lines never reach them. Put toolchain binaries on that PATH (for example symlinks \
in /usr/local/bin), world-readable and never under /root.";

/// `minos judge push`'s long help: what is packed, where it goes, and the label rule (07-CONTEXT D-01 to D-03).
const JUDGE_PUSH_HELP: &str = "\
Register a directory of judge data under the manifest's [judge] version label.

The directory is packed with the source snapshot's hygiene and no override: its own .gitignore and .minosignore \
files apply, the hard denylist (.git/, .minos/, .env*, keys, ...) drops entries and every drop is listed, \
symlinks and special files refuse the push, and the secret scan must pass. Data over 96 MiB is refused, never \
truncated. Every path the judge command names must be in the directory, at the same project-relative path. A \
directory inside the project must lie under a [source] exclude entry.

The tarball is stored in .minos/blobs, and minos.lock records the label, its tree digest, the file count and \
bytes, and each file's path and SHA-256, never the data. Committing minos.lock exposes the judge file paths, and \
a low-entropy file can be guessed from its digest. A fresh clone must push again. Different data under a label \
minos.lock already records is refused unless --update is passed; bump [judge] version for new data instead.";

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
        .subcommand(
            Command::new("judge")
                .about("Manage the project's judge data")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("push")
                        .about("register judge data under the manifest's [judge] version label")
                        .long_about(JUDGE_PUSH_HELP)
                        .arg(
                            Arg::new("dir")
                                .required(true)
                                .value_parser(clap::value_parser!(std::path::PathBuf))
                                .help(
                                    "the directory of judge data to push under the manifest's [judge] version label",
                                ),
                        )
                        .arg(
                            Arg::new("update")
                                .long("update")
                                .action(ArgAction::SetTrue)
                                .help("replace the judge data already recorded for this label"),
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
        Some(("judge", judge)) => match judge.subcommand() {
            Some(("push", push)) => judge_push_command(push),
            _ => Err(anyhow!("unknown judge command")),
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
            // What the packer kept or changed in the user's image (review WR-01): setid bits are kept but
            // inert, because the guest mounts the root nosuid; the image's own /init is replaced by the guest.
            println!(
                "setid files kept: {} (inert: the guest mounts the root nosuid)",
                pack.setid_files()
            );
            println!("hardlinks copied: {}", pack.hardlinks_copied());
            println!(
                "/init: {}",
                if pack.init_overridden() {
                    "replaced the image's own /init"
                } else {
                    "added"
                }
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

/// Runs `minos judge push` for the project in the current directory and prints what was recorded. Every path
/// printed goes through [`SafeText`].
fn judge_push_command(matches: &ArgMatches) -> anyhow::Result<ExitCode> {
    let project = env::current_dir().context("reading the current directory")?;
    let dir = matches
        .get_one::<std::path::PathBuf>("dir")
        .ok_or_else(|| anyhow!("the judge data directory is required"))?;
    let options = JudgePushOptions::new(matches.get_flag("update"));
    let report = judge_push(&project, dir, options).with_context(|| {
        format!(
            "pushing judge data from {}",
            SafeText::inline(&dir.to_string_lossy())
        )
    })?;
    println!(
        "judge data: label {}, sha256 {}",
        report.label(),
        report.tree()
    );
    println!("files: {}, {} bytes", report.file_count(), report.bytes());
    println!("judge version: {}", report.judge_version());
    println!(
        "minos.lock: {}",
        if report.lock_written() {
            "written"
        } else {
            "unchanged"
        }
    );
    for denied in report.denied() {
        println!(
            "dropped by the denylist: {} ({})",
            denied.path(),
            denied.rule().pattern()
        );
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::cli;

    #[test]
    fn command_tree_is_well_formed() {
        cli().debug_assert();
    }

    #[test]
    fn judge_push_parses_dir_and_update() {
        let parsed = cli().try_get_matches_from(["minos", "judge", "push", "data", "--update"]);
        assert!(parsed.is_ok(), "{parsed:?}");
        let missing = cli().try_get_matches_from(["minos", "judge", "push"]);
        assert!(missing.is_err(), "push without a directory parsed");
    }
}
