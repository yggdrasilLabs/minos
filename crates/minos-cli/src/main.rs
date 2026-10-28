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
//! - `minos init --kind <tune|optimize|explore>`: scaffold a project and a price table (CLI-01;
//!   [`minos_cli::init`]).
//! - `minos check [--fresh] [--allow-dirty]`: validate the project, calibrate the baseline, and print the routes
//!   and advisories (CLI-05; [`minos_cli::check`]).
//! - `minos run --generations <N> [--run <prefix>] [--allow-dirty]`: start or resume a run (CLI-06;
//!   [`minos_cli::run`]).
//! - `minos stop [<run>]`: ask the driver holding a run to stop (CLI-10; [`minos_cli::stop`]).
//! - `minos status`, `minos log`, `minos show <exp>`: read a run's ledger and blobs (CLI-07; [`minos_cli::status`],
//!   [`minos_cli::log`], [`minos_cli::show`]).
//! - `minos report <exp>` and `minos adopt <exp> --reviewed <hash>`: the review gate (CLI-08, CLI-09;
//!   [`minos_cli::report`], [`minos_cli::adopt`]).
//!
//! This file is the only place the concrete types meet: it picks [`minos_propose::ClaudeCli`] and the krun backend
//! ([`minos_cli::jobs::krun_jobs`]), reads the clock and `HOME`, and maps each command's outcome to an exit code.
//! Every command's logic lives in the library. `anyhow` appears only here, at the binary boundary
//! (rules/err-anyhow-app).

mod host;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Arg, ArgAction, ArgMatches, Command};
use minos_cli::address::HexPrefix;
use minos_cli::builder::DockerBuildx;
use minos_cli::env_build::{EnvBuildError, EnvBuildOptions, env_build};
use minos_cli::init::InitKind;
use minos_cli::judge_push::{JudgePushOptions, judge_push};
use minos_cli::locate::locate;
use minos_core::id::ReportHash;
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

/// `minos init`'s long help.
const INIT_HELP: &str = "\
Scaffold a project of the given kind in the current directory: minos.toml, program.md, minos.Dockerfile, and a \
.gitignore for the files Minos writes. Nothing is overwritten: if any of the four exists, init refuses, names \
every one, and writes nothing. It also writes a price table at ~/.config/minos/prices.toml when none is there, \
with the compute rates and a commented model table; a run that names a model refuses to start until that model \
is priced, so no run is ever priced at zero. An existing price table is kept unchanged.";

/// `minos check`'s long help (08-CONTEXT D-05 to D-09).
const CHECK_HELP: &str = "\
Validate the project and calibrate its baseline before a run.

check reads minos.toml, requires minos.lock, the root image, and the pushed judge data (naming minos env build or \
minos judge push when one is missing or stale), packs the source snapshot, and runs the claude CLI preflight when \
a proposer names a model. It prints the manifest's facts, the memory budget, the judge-scratch requirement, the \
route table with its reasons, and an advisory when a single_shot batch may reach the CLI's output cap.

It runs the baseline [judge] trials times in a diagnostic run and prints each trial, the noise floor, and \
min_improvement beside the noise verdict. A diagnostic run already calibrated for the same manifest, base, root \
image, and judge is reused, launching no VM, unless --fresh is passed. The noise verdict is advice: check never \
refuses on noise. It exits 1 only when something refuses.";

/// `minos run`'s long help (08-CONTEXT D-14; 07-CONTEXT D-14, D-19).
const RUN_HELP: &str = "\
Start a run, or resume one with --run, and drive it to a generation.

--generations is the absolute generation to reach, not a count to add, so a resumed run ends where an \
uninterrupted one would. Every run calibrates its own baseline as generation 0 first. A proposer that names a \
model needs the claude CLI preflight to pass before anything starts. A resume is refused when the base, the root \
image, the judge version, or the experiment frame changed; that needs a new run.

Progress is one line per recorded trial, one per generation's decision, and one approx. cost line per generation. \
The exit status is 0 when the target generation is reached or a minos stop ended the session, and 2 when a spend \
or token cap, the circuit breaker, or the model provider stopped it.";

/// `minos stop`'s long help (08-CONTEXT D-13).
const STOP_HELP: &str = "\
Ask the driver holding a run (the latest run when none is named) to stop.

A stop is recorded only while a driver holds the run; with none, stop exits 1 and records nothing, because a stop \
recorded with no session running would never take effect. The driver stops at its next safe point. It never \
kills a VM, so a trial already running finishes and is recorded first.";

/// `minos status`'s long help (08-CONTEXT D-15).
const STATUS_HELP: &str = "\
Print a run's progress (the latest run when --run is not given): when it started, the generations concluded, the \
incumbent against the baseline and the noise floor, the approx. cost with token counts, the recorded stops, and \
whether a driver holds the run now. It appends nothing to the ledger. Its liveness probe takes and at once \
releases the run's driver lock, which creates the run's empty driver.lock if no driver has locked the run \
before. A ledger that is corrupt or does not fold is reported, and status exits 1: it needs a human.";

/// `minos log`'s long help (08-CONTEXT D-16).
const LOG_HELP: &str = "\
Print a run's ledger, one line per event in order (the latest run when --run is not given), with IDs as 12-hex \
prefixes. --generation and --exp keep only the events of one generation or one experiment. --json prints each \
stored line as the ledger holds it.";

/// `minos show`'s long help (08-CONTEXT D-17).
const SHOW_HELP: &str = "\
Print one experiment: its parent, generation, tier, and origin; every trial's outcome; the lineage from the \
baseline; the rationale; and the cumulative diff from the run's base. Stage logs print only with --logs, so a \
hostile log never prints unasked. Every string from a sandbox or a model is sanitized before it is printed.";

/// `minos report`'s long help (08-CONTEXT D-11).
const REPORT_HELP: &str = "\
Build the review report for one experiment from the ledger and blobs and print it with its report hash and the \
minos adopt command that accepts that hash. It appends nothing. Read the report before adopting: its flags direct \
attention, they are not verdicts.";

/// `minos adopt`'s long help (08-CONTEXT D-11, D-12).
const ADOPT_HELP: &str = "\
Accept a reviewed experiment and write its patch file.

--reviewed must be the report hash minos report printed for that experiment. adopt rebuilds the report now and \
refuses when it hashes to anything else, so a report that changed since it was read must be read again. It \
records the acknowledgment in the ledger, then writes the cumulative diff from the run's base to \
minos-<experiment prefix>.patch in the project, or to --out, creating a new file and never overwriting one. It \
never applies the patch: it prints the git apply and patch -p1 commands that would.";

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
        .subcommand(init_command())
        .subcommand(check_command())
        .subcommand(run_command())
        .subcommand(stop_command())
        .subcommand(status_command())
        .subcommand(log_command())
        .subcommand(show_command())
        .subcommand(report_command())
        .subcommand(adopt_command())
}

/// Parses a run or experiment prefix: 1 to 64 lowercase hex characters.
fn hex_prefix(text: &str) -> Result<HexPrefix, minos_cli::address::AddressError> {
    HexPrefix::from_str(text)
}

/// Parses a report hash: 64 lowercase hex characters.
fn report_hash(text: &str) -> Result<ReportHash, minos_core::id::IdParseError> {
    ReportHash::from_str(text)
}

/// A boolean flag.
fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::SetTrue)
        .help(help)
}

/// `--run <prefix>`: the run to read, by unique hex prefix; the latest run when absent.
fn run_arg() -> Arg {
    Arg::new("run")
        .long("run")
        .value_name("PREFIX")
        .value_parser(hex_prefix)
        .help("the run, by unique hex prefix of its ID; the latest run when absent")
}

/// `<exp>`: the experiment, by unique hex prefix within the run.
fn exp_arg() -> Arg {
    Arg::new("exp")
        .required(true)
        .value_name("EXP")
        .value_parser(hex_prefix)
        .help("the experiment, by unique hex prefix of its ID within the run")
}

/// `--full-ids`: print whole IDs instead of 12-hex prefixes.
fn full_ids_arg() -> Arg {
    flag("full-ids", "print whole IDs instead of 12-hex prefixes")
}

/// `--allow-dirty`: skip only the clean check.
fn allow_dirty_arg() -> Arg {
    flag(
        "allow-dirty",
        "skip only the clean check; the denylist and the secret scan still run",
    )
}

/// `minos init`.
fn init_command() -> Command {
    Command::new("init")
        .about("Scaffold a project and a price table")
        .long_about(INIT_HELP)
        .arg(
            Arg::new("kind")
                .long("kind")
                .required(true)
                .value_parser(
                    PossibleValuesParser::new(["tune", "optimize", "explore"])
                        .try_map(|kind| InitKind::from_str(&kind)),
                )
                .help("the kind of project: tune, optimize, or explore"),
        )
}

/// `minos check`.
fn check_command() -> Command {
    Command::new("check")
        .about("Validate the project, calibrate the baseline, and print the routes")
        .long_about(CHECK_HELP)
        .arg(flag(
            "fresh",
            "start a new diagnostic run instead of reusing a calibrated one",
        ))
        .arg(allow_dirty_arg())
}

/// `minos run`.
fn run_command() -> Command {
    Command::new("run")
        .about("Start or resume a run")
        .long_about(RUN_HELP)
        .arg(
            Arg::new("generations")
                .long("generations")
                .required(true)
                .value_name("N")
                .value_parser(clap::value_parser!(u32).range(1..))
                .help("the absolute generation to reach, at least 1"),
        )
        .arg(
            Arg::new("run")
                .long("run")
                .value_name("PREFIX")
                .value_parser(hex_prefix)
                .help("resume the run with this unique hex prefix instead of starting a new one"),
        )
        .arg(allow_dirty_arg())
}

/// `minos stop`.
fn stop_command() -> Command {
    Command::new("stop")
        .about("Ask the driver holding a run to stop at its next safe point")
        .long_about(STOP_HELP)
        .arg(
            Arg::new("run")
                .value_name("RUN")
                .value_parser(hex_prefix)
                .help("the run, by unique hex prefix of its ID; the latest run when absent"),
        )
}

/// `minos status`.
fn status_command() -> Command {
    Command::new("status")
        .about("Print a run's progress, liveness, and ledger health")
        .long_about(STATUS_HELP)
        .arg(run_arg())
        .arg(full_ids_arg())
}

/// `minos log`.
fn log_command() -> Command {
    Command::new("log")
        .about("Print a run's ledger events")
        .long_about(LOG_HELP)
        .arg(run_arg())
        .arg(
            Arg::new("generation")
                .long("generation")
                .value_name("N")
                .value_parser(clap::value_parser!(u32))
                .help("keep only this generation's events"),
        )
        .arg(
            Arg::new("exp")
                .long("exp")
                .value_name("PREFIX")
                .value_parser(hex_prefix)
                .help("keep only the events naming this experiment, by unique hex prefix"),
        )
        .arg(flag("json", "print each stored ledger line as JSON"))
        .arg(full_ids_arg())
}

/// `minos show`.
fn show_command() -> Command {
    Command::new("show")
        .about("Print one experiment: trials, lineage, rationale, and diff")
        .long_about(SHOW_HELP)
        .arg(exp_arg())
        .arg(run_arg())
        .arg(flag("logs", "also print each stage's stored log tail"))
        .arg(full_ids_arg())
}

/// `minos report`.
fn report_command() -> Command {
    Command::new("report")
        .about("Print the review report for one experiment and its hash")
        .long_about(REPORT_HELP)
        .arg(exp_arg())
        .arg(run_arg())
}

/// `minos adopt`.
fn adopt_command() -> Command {
    Command::new("adopt")
        .about("Acknowledge a reviewed report and write its patch file")
        .long_about(ADOPT_HELP)
        .arg(exp_arg())
        .arg(
            Arg::new("reviewed")
                .long("reviewed")
                .required(true)
                .value_name("HASH")
                .value_parser(report_hash)
                .help("the report hash minos report printed for this experiment"),
        )
        .arg(run_arg())
        .arg(
            Arg::new("out")
                .long("out")
                .value_name("PATH")
                .value_parser(clap::value_parser!(PathBuf))
                .help("write the patch here instead of minos-<experiment prefix>.patch in the project"),
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
    use clap::Command;

    use super::cli;

    /// A report hash: 64 lowercase hex characters.
    const HASH: &str = "abababababababababababababababababababababababababababababababab";

    /// Whether `args`, after `minos`, parse.
    fn parses(args: &[&str]) -> Result<(), clap::Error> {
        cli()
            .try_get_matches_from(std::iter::once("minos").chain(args.iter().copied()))
            .map(drop)
    }

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

    #[test]
    fn phase_8_commands_parse_with_their_flags() {
        let accepted: [&[&str]; 11] = [
            &["run", "--generations", "3"],
            &["run", "--generations", "3", "--run", "ab12"],
            &["check", "--fresh", "--allow-dirty"],
            &["stop"],
            &["stop", "ab12"],
            &["status", "--run", "ab", "--full-ids"],
            &["log", "--generation", "2", "--exp", "ab", "--json"],
            &["show", "ab", "--logs"],
            &["report", "ab", "--run", "cd"],
            &["adopt", "ab", "--reviewed", HASH, "--out", "x.patch"],
            &["init", "--kind", "tune"],
        ];
        for args in accepted {
            let parsed = parses(args);
            assert!(parsed.is_ok(), "{args:?}: {parsed:?}");
        }
    }

    #[test]
    fn phase_8_commands_refuse_bad_arguments() {
        let refused: [&[&str]; 8] = [
            &["run"],
            &["run", "--generations", "0"],
            &["run", "--generations", "2", "--run", "XYZ"],
            &["adopt", "ab"],
            &["adopt", "ab", "--reviewed", "nothex"],
            &["init", "--kind", "bogus"],
            &["show"],
            &["log", "--generation", "two"],
        ];
        for args in refused {
            assert!(parses(args).is_err(), "{args:?} parsed");
        }
    }

    /// Whether `text` holds the word estimate anywhere or the whole word exact, case-insensitively.
    fn banned_word(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if lower.contains("estimate") {
            return Some("estimate");
        }
        lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == "exact")
            .then_some("exact")
    }

    /// Checks the long help of `command` and of every command under it, recursively.
    fn check_help(command: &Command, path: &str, checked: &mut Vec<String>) {
        let name = format!("{path} {}", command.get_name());
        let help = command.clone().render_long_help().to_string();
        assert!(
            banned_word(&help).is_none(),
            "{name}'s help says {:?}:\n{help}",
            banned_word(&help)
        );
        checked.push(name.clone());
        for sub in command.get_subcommands() {
            check_help(sub, &name, checked);
        }
    }

    #[test]
    fn no_help_text_says_estimate_or_exact() {
        let mut root = cli();
        root.build();
        let mut checked = Vec::new();
        check_help(&root, "", &mut checked);
        for command in [
            "init", "check", "run", "stop", "status", "log", "show", "report", "adopt",
        ] {
            let name = format!(" minos {command}");
            assert!(
                checked.contains(&name),
                "{name} was not checked: {checked:?}"
            );
        }
    }
}
