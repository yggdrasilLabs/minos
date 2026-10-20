//! The `claude` preflight on the real spawn path (06-CONTEXT D-01 to D-04 as amended and extended; PROP-02,
//! PROP-04), against a fake `claude` in a temporary `HOME`. No test runs the user's own `claude`, makes a model
//! call, or needs a login.
//!
//! The fake is a `/bin/sh` script of fixed test text installed in the native layout:
//! `$HOME/.local/share/claude/versions/<version>` with `$HOME/.local/bin/claude` linking to it. On `--version` it
//! prints `$HOME/fake/version.out` when that file exists, else its own file name and ` (Claude Code)`, and exits
//! with `$HOME/fake/version.code` or 0. On `auth status` it prints `$HOME/fake/auth.out` and exits with
//! `$HOME/fake/auth.code` or 0. Each step first appends to `$HOME/fake/<step>.count` and records its
//! environment, its canonical working directory, and that directory's listing.
//!
//! Tests cannot set process variables in edition 2024, so each builds its [`ParentEnv`] by hand (06-RESEARCH
//! Pitfall 6).

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use minos_propose::claude_cli::command::ClaudeVersion;
use minos_propose::claude_cli::env::{EnvError, ParentEnv};
use minos_propose::claude_cli::preflight::{PreflightError, Preflighted, preflight};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

/// The fake `claude`. Fixed test text, never built from input.
const FAKE_CLAUDE: &str = r#"#!/bin/sh
fake="$HOME/fake"
case "$1" in
  --version) step=version ;;
  auth) step=auth ;;
  *) exit 64 ;;
esac
echo ran >> "$fake/$step.count"
/usr/bin/env > "$fake/$step.env"
/bin/pwd -P > "$fake/$step.pwd"
/bin/ls -A > "$fake/$step.ls"
if [ "$step" = version ]; then
  if [ -f "$fake/version.out" ]; then
    /bin/cat "$fake/version.out"
  else
    printf '%s (Claude Code)\n' "$(/usr/bin/basename "$0")"
  fi
  if [ -f "$fake/version.code" ]; then exit "$(/bin/cat "$fake/version.code")"; fi
  exit 0
fi
if [ -f "$fake/auth.out" ]; then /bin/cat "$fake/auth.out"; fi
if [ -f "$fake/auth.code" ]; then exit "$(/bin/cat "$fake/auth.code")"; fi
exit 0
"#;

/// A clean subscription login, with the email and organization fields the CLI also prints.
const LOGGED_IN: &str = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"someone@example.com","orgId":"org-123"}"#;

/// Variables `/bin/sh` itself puts in the environment of the programs it runs; the fake's recorded environment
/// is compared without them, because the provider's child is the script, not the programs the script runs.
const SHELL_SET: [&str; 3] = ["PWD", "SHLVL", "_"];

/// A temporary `HOME` holding the fake in the native layout, and a separate temporary `TMPDIR`.
struct Fixture {
    home: TempDir,
    tmp: TempDir,
}

impl Fixture {
    /// The fake installed as `version` and linked, logged in with a clean subscription.
    fn new(version: &str) -> Result<Self, Box<dyn Error>> {
        let fixture = Self::unlinked(version)?;
        symlink(
            fixture.binary(version),
            fixture.home().join(".local/bin/claude"),
        )?;
        Ok(fixture)
    }

    /// The fake installed as `version` but with no link.
    fn unlinked(version: &str) -> Result<Self, Box<dyn Error>> {
        let fixture = Self {
            home: TempDir::new()?,
            tmp: TempDir::new()?,
        };
        fs::create_dir_all(fixture.home().join(".local/bin"))?;
        fs::create_dir_all(fixture.home().join(".local/share/claude/versions"))?;
        fs::create_dir_all(fixture.fake())?;
        let binary = fixture.binary(version);
        fs::write(&binary, FAKE_CLAUDE)?;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
        fixture.write("auth.out", LOGGED_IN)?;
        Ok(fixture)
    }

    fn home(&self) -> &Path {
        self.home.path()
    }

    fn tmp(&self) -> &Path {
        self.tmp.path()
    }

    fn binary(&self, version: &str) -> PathBuf {
        self.home()
            .join(".local/share/claude/versions")
            .join(version)
    }

    fn fake(&self) -> PathBuf {
        self.home().join("fake")
    }

    /// Writes a control or canned-output file for the fake.
    fn write(&self, name: &str, contents: &str) -> TestResult {
        fs::write(self.fake().join(name), contents)?;
        Ok(())
    }

    /// A recorded file, or `None` when the fake never wrote it.
    fn recorded(&self, name: &str) -> Result<Option<String>, Box<dyn Error>> {
        match fs::read_to_string(self.fake().join(name)) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// How many times the fake ran `step`.
    fn runs(&self, step: &str) -> Result<usize, Box<dyn Error>> {
        Ok(self
            .recorded(&format!("{step}.count"))?
            .map_or(0, |text| text.lines().count()))
    }

    /// The parent environment of a user with this `HOME` and `TMPDIR`, plus `extra`.
    fn parent(&self, extra: &[(&str, &str)]) -> ParentEnv {
        parent_env(Some(self.home()), self.tmp(), extra)
    }
}

/// A parent environment with `USER` and `LOGNAME` `tester`, the given `HOME` and `TMPDIR`, and `extra`.
fn parent_env(home: Option<&Path>, tmp: &Path, extra: &[(&str, &str)]) -> ParentEnv {
    let mut vars: Vec<(OsString, OsString)> = vec![
        (OsString::from("USER"), OsString::from("tester")),
        (OsString::from("LOGNAME"), OsString::from("tester")),
        (OsString::from("TMPDIR"), tmp.as_os_str().to_owned()),
    ];
    if let Some(home) = home {
        vars.push((OsString::from("HOME"), home.as_os_str().to_owned()));
    }
    vars.extend(
        extra
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    );
    ParentEnv::from_vars(vars)
}

fn refused(result: Result<Preflighted, PreflightError>) -> Result<PreflightError, Box<dyn Error>> {
    match result {
        Ok(passed) => Err(format!("expected a refusal, got {passed:?}").into()),
        Err(error) => Ok(error),
    }
}

/// The recorded environment of `step` without the variables the shell adds, sorted.
fn recorded_env(fixture: &Fixture, step: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let text = fixture
        .recorded(&format!("{step}.env"))?
        .ok_or("the fake recorded no environment")?;
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| {
            !SHELL_SET
                .iter()
                .any(|name| line.split_once('=').is_some_and(|(key, _)| key == *name))
        })
        .map(str::to_owned)
        .collect();
    lines.sort_unstable();
    Ok(lines)
}

#[test]
fn a_clean_subscription_login_passes() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    let passed = preflight(&fixture.parent(&[]))?;
    assert_eq!(passed.located().version(), ClaudeVersion::new(2, 1, 304));
    assert_eq!(
        passed.located().path(),
        fs::canonicalize(fixture.binary("2.1.304"))?.as_path()
    );
    assert_eq!(
        passed.work_root(),
        fs::canonicalize(fixture.tmp())?.as_path()
    );
    assert_eq!(passed.home(), fixture.home());
    assert_eq!(passed.child_env().len(), 6);
    let debug = format!("{passed:?}");
    assert!(
        debug.contains("2.1.304") || debug.contains("304"),
        "{debug}"
    );
    assert!(!debug.contains("someone@example.com"), "{debug}");
    assert!(!debug.contains("org-123"), "{debug}");
    Ok(())
}

#[test]
fn each_query_runs_once_in_a_fresh_empty_directory_with_six_variables() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    preflight(&fixture.parent(&[]))?;
    let root = fs::canonicalize(fixture.tmp())?;
    let home = fixture.home().display().to_string();
    let tmp = fixture.tmp().display().to_string();
    let expected = [
        format!("HOME={home}"),
        "LANG=en_US.UTF-8".to_owned(),
        "LOGNAME=tester".to_owned(),
        "PATH=/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
        format!("TMPDIR={tmp}"),
        "USER=tester".to_owned(),
    ];
    let mut dirs = Vec::new();
    for step in ["version", "auth"] {
        assert_eq!(fixture.runs(step)?, 1, "{step}");
        assert_eq!(recorded_env(&fixture, step)?, expected, "{step}");
        let pwd = fixture
            .recorded(&format!("{step}.pwd"))?
            .ok_or("the fake recorded no directory")?;
        let dir = PathBuf::from(pwd.trim_end());
        assert_eq!(
            dir.parent(),
            Some(root.as_path()),
            "{step}: {}",
            dir.display()
        );
        let name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("the directory has no name")?;
        assert!(name.starts_with("minos-claude-"), "{name}");
        assert!(!dir.exists(), "{} was not removed", dir.display());
        assert_eq!(
            fixture.recorded(&format!("{step}.ls"))?.as_deref(),
            Some(""),
            "{step}: the directory was not empty"
        );
        dirs.push(dir);
    }
    assert_ne!(dirs.first(), dirs.get(1), "both queries shared a directory");
    Ok(())
}

#[test]
fn credential_variables_are_refused_by_name_before_anything_runs() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    let error = refused(preflight(&fixture.parent(&[
        ("ANTHROPIC_API_KEY", "sk-secret-value"),
        ("CLAUDE_CODE_USE_BEDROCK", "1"),
    ])))?;
    match &error {
        PreflightError::CredentialVariables { names } => {
            assert_eq!(names, &["ANTHROPIC_API_KEY", "CLAUDE_CODE_USE_BEDROCK"]);
        }
        other => return Err(format!("expected CredentialVariables, got {other:?}").into()),
    }
    let message = error.to_string();
    assert!(message.contains("ANTHROPIC_API_KEY"), "{message}");
    assert!(message.contains("CLAUDE_CODE_USE_BEDROCK"), "{message}");
    assert!(!message.contains("sk-secret-value"), "{message}");
    assert!(!format!("{error:?}").contains("sk-secret-value"));
    assert!(!error.is_auth());
    assert_eq!(fixture.runs("version")?, 0);
    assert_eq!(fixture.runs("auth")?, 0);
    Ok(())
}

#[test]
fn a_missing_home_is_an_environment_refusal() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    let error = refused(preflight(&parent_env(None, fixture.tmp(), &[])))?;
    assert!(
        matches!(
            error,
            PreflightError::Environment(EnvError::Missing { name: "HOME" })
        ),
        "{error:?}"
    );
    assert_eq!(fixture.runs("version")?, 0);
    Ok(())
}

#[test]
fn no_link_is_a_locate_refusal_and_nothing_runs() -> TestResult {
    let fixture = Fixture::unlinked("2.1.304")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert!(matches!(error, PreflightError::Locate(_)), "{error:?}");
    assert_eq!(fixture.runs("version")?, 0);
    assert_eq!(fixture.runs("auth")?, 0);
    Ok(())
}

#[test]
fn a_version_below_the_minimum_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.295")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert_eq!(
        error,
        PreflightError::VersionTooOld {
            found: ClaudeVersion::new(2, 1, 295),
            minimum: ClaudeVersion::new(2, 1, 296),
        }
    );
    assert_eq!(fixture.runs("auth")?, 0);
    Ok(())
}

#[test]
fn a_version_that_disagrees_with_the_file_name_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write("version.out", "2.1.305 (Claude Code)\n")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert_eq!(
        error,
        PreflightError::VersionMismatch {
            file: ClaudeVersion::new(2, 1, 304),
            reported: ClaudeVersion::new(2, 1, 305),
        }
    );
    Ok(())
}

#[test]
fn unparseable_version_output_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write("version.out", "claude, some version\n")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert!(
        matches!(error, PreflightError::VersionUnreadable { .. }),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn a_failing_version_query_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write("version.code", "2")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert!(
        matches!(error, PreflightError::VersionUnreadable { .. }),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn auth_status_exit_one_is_not_logged_in() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write(
        "auth.out",
        r#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty"}"#,
    )?;
    fixture.write("auth.code", "1")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert_eq!(error, PreflightError::NotLoggedIn);
    assert!(error.is_auth());
    Ok(())
}

#[test]
fn logged_in_false_is_not_logged_in() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write("auth.out", r#"{"loggedIn":false}"#)?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    assert_eq!(error, PreflightError::NotLoggedIn);
    Ok(())
}

#[test]
fn another_auth_status_exit_is_a_failure() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write("auth.out", "something broke")?;
    fixture.write("auth.code", "3")?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    match &error {
        PreflightError::AuthStatusFailed { code, detail } => {
            assert_eq!(*code, Some(3));
            assert_eq!(detail.as_str(), "something broke");
        }
        other => return Err(format!("expected AuthStatusFailed, got {other:?}").into()),
    }
    assert!(!error.is_auth());
    Ok(())
}

#[test]
fn an_api_key_login_is_not_a_subscription() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write(
        "auth.out",
        r#"{"loggedIn":true,"authMethod":"api_key","apiProvider":"firstParty"}"#,
    )?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    match &error {
        PreflightError::NotSubscriptionLogin { method } => assert_eq!(method.as_str(), "api_key"),
        other => return Err(format!("expected NotSubscriptionLogin, got {other:?}").into()),
    }
    assert!(error.is_auth());
    Ok(())
}

#[test]
fn a_third_party_provider_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fixture.write(
        "auth.out",
        r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"bedrock"}"#,
    )?;
    let error = refused(preflight(&fixture.parent(&[])))?;
    match &error {
        PreflightError::NotFirstParty { provider } => assert_eq!(provider.as_str(), "bedrock"),
        other => return Err(format!("expected NotFirstParty, got {other:?}").into()),
    }
    assert!(error.is_auth());
    Ok(())
}

#[test]
fn an_api_key_source_is_refused_whatever_its_value() -> TestResult {
    for source in [r#""/usr/local/bin/key-helper""#, "null"] {
        let fixture = Fixture::new("2.1.304")?;
        fixture.write(
            "auth.out",
            &format!(
                r#"{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","apiKeySource":{source}}}"#
            ),
        )?;
        let error = refused(preflight(&fixture.parent(&[])))?;
        assert_eq!(error, PreflightError::ApiKeySourcePresent, "{source}");
        assert!(error.is_auth());
    }
    Ok(())
}

#[test]
fn non_json_auth_status_is_unreadable() -> TestResult {
    for output in [
        "Logged in as someone",
        r#"[true,"claude.ai","firstParty"]"#,
        "",
    ] {
        let fixture = Fixture::new("2.1.304")?;
        fixture.write("auth.out", output)?;
        let error = refused(preflight(&fixture.parent(&[])))?;
        assert_eq!(error, PreflightError::AuthStatusUnreadable, "{output}");
        assert!(!error.is_auth());
    }
    Ok(())
}

#[test]
fn a_work_root_inside_a_repository_is_refused() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    let repo = TempDir::new()?;
    fs::create_dir(repo.path().join(".git"))?;
    let tmp = repo.path().join("nested/tmp");
    fs::create_dir_all(&tmp)?;
    let error = refused(preflight(&parent_env(Some(fixture.home()), &tmp, &[])))?;
    assert_eq!(
        error,
        PreflightError::WorkRootInRepository {
            path: fs::canonicalize(repo.path())?,
            marker: ".git",
        }
    );
    let message = error.to_string();
    assert!(message.contains(".git"), "{message}");
    assert_eq!(fixture.runs("version")?, 0);
    Ok(())
}

#[test]
fn project_configuration_above_the_work_root_is_refused() -> TestResult {
    for marker in ["CLAUDE.md", ".mcp.json", ".claude"] {
        let fixture = Fixture::new("2.1.304")?;
        let project = TempDir::new()?;
        fs::write(project.path().join(marker), "")?;
        let tmp = project.path().join("tmp");
        fs::create_dir(&tmp)?;
        let error = refused(preflight(&parent_env(Some(fixture.home()), &tmp, &[])))?;
        assert_eq!(
            error,
            PreflightError::WorkRootInRepository {
                path: fs::canonicalize(project.path())?,
                marker,
            },
            "{marker}"
        );
    }
    Ok(())
}

#[test]
fn the_users_own_claude_directory_does_not_refuse_a_work_root_under_home() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    fs::create_dir(fixture.home().join(".claude"))?;
    let tmp = fixture.home().join("tmp");
    fs::create_dir(&tmp)?;
    let passed = preflight(&parent_env(Some(fixture.home()), &tmp, &[]))?;
    assert_eq!(passed.work_root(), fs::canonicalize(&tmp)?.as_path());
    Ok(())
}

#[test]
fn a_missing_work_root_is_unusable() -> TestResult {
    let fixture = Fixture::new("2.1.304")?;
    let tmp = fixture.tmp().join("does-not-exist");
    let error = refused(preflight(&parent_env(Some(fixture.home()), &tmp, &[])))?;
    assert_eq!(error, PreflightError::WorkRootUnusable { path: tmp });
    Ok(())
}
