//! The `minos` binary's own wiring, run as a process: the command tree, `init`, and the read-only commands'
//! refusals in a project with no runs. No VM boots and no model is called: every command here either needs no
//! provider and no backend, or refuses before reaching one.
//!
//! Each invocation runs with a cleared environment holding only `PATH=/usr/bin:/bin` and a temporary `HOME`, in a
//! temporary project directory, and everything it prints to stdout and stderr is checked for the words estimate and
//! exact (COST-02).

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn Error>>;

/// The twelve top-level commands `minos --help` lists.
const COMMANDS: [&str; 12] = [
    "host", "env", "judge", "init", "check", "run", "stop", "status", "log", "show", "report",
    "adopt",
];

/// The four files `minos init` scaffolds in the project.
const SCAFFOLD: [&str; 4] = ["minos.toml", "program.md", "minos.Dockerfile", ".gitignore"];

/// A temporary `HOME` and an empty temporary project directory, both canonical.
struct Sandbox {
    _home: tempfile::TempDir,
    _project: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
}

impl Sandbox {
    fn new() -> Result<Self, Box<dyn Error>> {
        let home_dir = tempfile::tempdir()?;
        let project_dir = tempfile::tempdir()?;
        let home = fs::canonicalize(home_dir.path())?;
        let project = fs::canonicalize(project_dir.path())?;
        Ok(Self {
            _home: home_dir,
            _project: project_dir,
            home,
            project,
        })
    }

    /// Runs `minos args` in the project with the cleared environment, and checks both streams for banned words.
    fn minos(&self, args: &[&str]) -> Result<Output, Box<dyn Error>> {
        let output = Command::new(env!("CARGO_BIN_EXE_minos"))
            .args(args)
            .current_dir(&self.project)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.home)
            .output()?;
        assert_no_banned_words(&String::from_utf8_lossy(&output.stdout))?;
        assert_no_banned_words(&String::from_utf8_lossy(&output.stderr))?;
        Ok(output)
    }

    /// The price table's path under the temporary `HOME`.
    fn prices(&self) -> PathBuf {
        self.home.join(".config").join("minos").join("prices.toml")
    }
}

/// Fails when `text` holds the word estimate anywhere, or the whole word exact, case-insensitively (COST-02).
fn assert_no_banned_words(text: &str) -> TestResult {
    let lower = text.to_lowercase();
    if lower.contains("estimate") {
        return Err(format!("the text contains the word estimate: {text}").into());
    }
    if lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "exact")
    {
        return Err(format!("the text contains the word exact: {text}").into());
    }
    Ok(())
}

/// Whether a regular file is at `path`.
fn is_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
}

#[test]
fn help_lists_every_command() -> TestResult {
    let sandbox = Sandbox::new()?;
    let output = sandbox.minos(&["--help"])?;
    let help = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "minos --help failed: {help}");
    for command in COMMANDS {
        assert!(
            help.lines()
                .any(|line| line.split_whitespace().next() == Some(command)),
            "{command} is not listed in:\n{help}"
        );
    }
    Ok(())
}

#[test]
fn every_command_help_runs_clean() -> TestResult {
    let sandbox = Sandbox::new()?;
    for command in COMMANDS {
        let output = sandbox.minos(&[command, "--help"])?;
        assert!(
            output.status.success(),
            "minos {command} --help failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn init_scaffolds_once_then_refuses() -> TestResult {
    let sandbox = Sandbox::new()?;

    let first = sandbox.minos(&["init", "--kind", "optimize"])?;
    let stdout = String::from_utf8(first.stdout)?;
    assert_eq!(
        first.status.code(),
        Some(0),
        "{stdout}{}",
        String::from_utf8_lossy(&first.stderr)
    );
    for name in SCAFFOLD {
        let path = sandbox.project.join(name);
        assert!(is_file(&path), "{name} was not written");
        assert!(
            stdout.contains(&format!("wrote {}", path.display())),
            "{name} is not reported in:\n{stdout}"
        );
    }
    assert!(
        is_file(&sandbox.prices()),
        "the price table was not written"
    );
    assert!(
        stdout.contains("wrote the price table"),
        "the price table is not reported in:\n{stdout}"
    );
    assert!(
        stdout.contains("minos env build, minos judge push judge, and minos check"),
        "no next steps in:\n{stdout}"
    );

    let second = sandbox.minos(&["init", "--kind", "optimize"])?;
    let stderr = String::from_utf8(second.stderr)?;
    assert_eq!(second.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("already exist"), "{stderr}");
    Ok(())
}

#[test]
fn init_keeps_an_existing_price_table() -> TestResult {
    let sandbox = Sandbox::new()?;
    let prices = sandbox.prices();
    let parent = prices.parent().ok_or("the price table has a parent")?;
    fs::create_dir_all(parent)?;
    fs::write(&prices, "# mine\n")?;

    let output = sandbox.minos(&["init", "--kind", "tune"])?;
    let stdout = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("kept the existing price table"), "{stdout}");
    assert_eq!(fs::read_to_string(&prices)?, "# mine\n");
    Ok(())
}

#[test]
fn status_without_runs_refuses() -> TestResult {
    let sandbox = Sandbox::new()?;
    let output = sandbox.minos(&["status"])?;
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("no runs recorded"), "{stderr}");
    assert!(
        !sandbox.project.join(".minos").exists(),
        "status created .minos"
    );
    Ok(())
}

#[test]
fn stop_without_runs_refuses() -> TestResult {
    let sandbox = Sandbox::new()?;
    let output = sandbox.minos(&["stop"])?;
    let stderr = String::from_utf8(output.stderr)?;
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("no runs recorded"), "{stderr}");
    assert!(
        !sandbox.project.join(".minos").exists(),
        "stop created .minos"
    );
    Ok(())
}

#[test]
fn read_only_commands_without_runs_refuse() -> TestResult {
    let sandbox = Sandbox::new()?;
    let hash = "ab".repeat(32);
    let commands: [&[&str]; 4] = [
        &["log"],
        &["show", "ab"],
        &["report", "ab"],
        &["adopt", "ab", "--reviewed", &hash],
    ];
    for args in commands {
        let output = sandbox.minos(args)?;
        let stderr = String::from_utf8(output.stderr)?;
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(stderr.contains("no runs recorded"), "{args:?}: {stderr}");
    }
    assert!(
        !sandbox.project.join(".minos").exists(),
        "a read-only command created .minos"
    );
    Ok(())
}

#[test]
fn bad_arguments_are_usage_errors() -> TestResult {
    let sandbox = Sandbox::new()?;
    let commands: [&[&str]; 3] = [
        &["run", "--generations", "0"],
        &["init", "--kind", "bogus"],
        &["status", "--run", "XYZ"],
    ];
    for args in commands {
        let output = sandbox.minos(args)?;
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
