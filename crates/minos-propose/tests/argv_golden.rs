//! The `claude -p` call contract (02-SPIKE-EVIDENCE.md SPIKE-07 contract; 06-CONTEXT D-03, D-04, D-06): the golden
//! argv with a byte pin, the parent-environment snapshot, the six-variable child environment, and version parsing.
//!
//! The argv is an insta snapshot under `tests/snapshots/`, one element per line. `.config/insta.yaml` sets
//! `update: no`, so the plain gate never writes or accepts one: the snapshot is created deliberately with
//! `INSTA_UPDATE=always cargo test -p minos-propose --test argv_golden` and reviewed before it is committed. insta
//! trims trailing whitespace and normalizes line endings when it compares, so `claude_argv_bytes_pinned` also pins
//! the exact bytes with one SHA-256 literal (03-RESEARCH Pitfall 4).

use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;

use minos_core::id::ModelId;
use minos_core::path::RelPath;
use minos_propose::claude_cli::command::{
    AUTH_STATUS_ARGS, ClaudeVersion, EMPTY_MCP_CONFIG, MIN_CLAUDE_VERSION, VERSION_ARGS, argv,
    parse_version_output,
};
use minos_propose::claude_cli::env::{
    CHILD_LANG, CHILD_PATH, EnvError, ParentEnv, REFUSED_PREFIX, REFUSED_VARIABLES,
};
use minos_propose::schema::OutputSchema;
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

fn haiku() -> Result<ModelId, Box<dyn Error>> {
    Ok("haiku".parse::<ModelId>()?)
}

fn schema() -> Result<OutputSchema, Box<dyn Error>> {
    let mut writable = BTreeSet::new();
    writable.insert(RelPath::try_from("src/parse.rs")?);
    Ok(OutputSchema::for_batch(4, &writable)?)
}

/// The argv as text, one element per line, each line ended by a newline.
fn argv_text() -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    for element in argv(&haiku()?, &schema()?) {
        let element = element
            .into_string()
            .map_err(|_| "an argv element is not UTF-8")?;
        text.push_str(&element);
        text.push('\n');
    }
    Ok(text)
}

fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    pairs
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect()
}

const FULL_PARENT: [(&str, &str); 4] = [
    ("HOME", "/h"),
    ("USER", "u"),
    ("LOGNAME", "u"),
    ("TMPDIR", "/t"),
];

#[test]
fn claude_argv() -> TestResult {
    let text = argv_text()?;
    insta::assert_snapshot!("claude_argv", text.trim_end_matches('\n'));
    Ok(())
}

#[test]
fn claude_argv_bytes_pinned() -> TestResult {
    // The pin was computed outside the code under test, from the accepted snapshot body (the text after the
    // file's second `---` line), which `awk` prints followed by one newline:
    //
    //   cd crates/minos-propose/tests/snapshots
    //   awk 'n >= 2 { print } /^---$/ { n++ }' argv_golden__claude_argv.snap | shasum -a 256
    //
    // so the bytes hashed here are the argv elements, each followed by one newline.
    assert_eq!(
        sha256_hex(argv_text()?.as_bytes()),
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    Ok(())
}

#[test]
fn argv_has_the_contract_shape() -> TestResult {
    let schema = schema()?;
    let argv = argv(&haiku()?, &schema);
    assert_eq!(argv.len(), 18);
    // Positions below are 0-based: element 5 of the contract (1-based) is index 4.
    assert_eq!(argv.get(1), Some(&OsString::from("--model")));
    assert_eq!(argv.get(2), Some(&OsString::from("haiku")));
    assert_eq!(argv.get(3), Some(&OsString::from("--tools")));
    assert_eq!(argv.get(4), Some(&OsString::new()));
    assert_eq!(argv.get(7), Some(&OsString::from("--json-schema")));
    assert_eq!(argv.get(8), Some(&OsString::from(schema.as_str())));
    assert_eq!(argv.get(11), Some(&OsString::from("--mcp-config")));
    assert_eq!(argv.get(12), Some(&OsString::from(EMPTY_MCP_CONFIG)));
    assert_eq!(EMPTY_MCP_CONFIG, r#"{"mcpServers":{}}"#);
    assert_eq!(argv.first(), Some(&OsString::from("-p")));
    assert_eq!(argv.last(), Some(&OsString::from("1")));
    Ok(())
}

#[test]
fn preflight_argv_constants() {
    assert_eq!(VERSION_ARGS, ["--version"]);
    assert_eq!(AUTH_STATUS_ARGS, ["auth", "status"]);
}

#[test]
fn parent_snapshot_records_refused_names_only() {
    let mut pairs = vars(&FULL_PARENT);
    pairs.extend(vars(&[
        ("ANTHROPIC_API_KEY", "sk-secret-value"),
        ("CLAUDE_CODE_USE_BEDROCK", "1"),
        ("FOO", "bar"),
    ]));
    let parent = ParentEnv::from_vars(pairs);
    let refused: Vec<&str> = parent.refused().iter().map(String::as_str).collect();
    assert_eq!(refused, ["ANTHROPIC_API_KEY", "CLAUDE_CODE_USE_BEDROCK"]);
    let debug = format!("{parent:?}");
    assert!(!debug.contains("sk-secret-value"), "{debug}");
    assert!(!debug.contains("bar"), "{debug}");
    assert!(debug.contains("ANTHROPIC_API_KEY"), "{debug}");
}

#[test]
fn every_named_variable_and_the_prefix_are_refused() {
    let mut pairs = vars(&FULL_PARENT);
    for name in REFUSED_VARIABLES {
        pairs.push((OsString::from(name), OsString::from("value")));
    }
    pairs.push((
        OsString::from("CLAUDE_CODE_USE_VERTEX"),
        OsString::from("1"),
    ));
    pairs.push((OsString::from("CLAUDE_CODE_USEX"), OsString::from("1")));
    let parent = ParentEnv::from_vars(pairs);
    assert_eq!(parent.refused().len(), REFUSED_VARIABLES.len() + 1);
    assert!(parent.refused().contains("CLAUDE_CONFIG_DIR"));
    assert!(parent.refused().contains("ANTHROPIC_BASE_URL"));
    assert!(parent.refused().contains("CLAUDE_CODE_USE_VERTEX"));
    assert!(!parent.refused().contains("CLAUDE_CODE_USEX"));
    assert_eq!(REFUSED_PREFIX, "CLAUDE_CODE_USE_");
}

#[test]
fn child_env_is_exactly_six_variables() -> TestResult {
    let mut pairs = vars(&FULL_PARENT);
    pairs.extend(vars(&[
        ("ANTHROPIC_API_KEY", "sk-secret-value"),
        ("PATH", "/opt/evil/bin"),
        ("LANG", "C"),
    ]));
    let parent = ParentEnv::from_vars(pairs);
    let env = parent.child_env()?;
    assert_eq!(
        env,
        vars(&[
            ("HOME", "/h"),
            ("LANG", "en_US.UTF-8"),
            ("LOGNAME", "u"),
            ("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
            ("TMPDIR", "/t"),
            ("USER", "u"),
        ])
    );
    assert_eq!(CHILD_PATH, "/usr/bin:/bin:/usr/sbin:/sbin");
    assert_eq!(CHILD_LANG, "en_US.UTF-8");
    Ok(())
}

#[test]
fn child_env_refuses_a_missing_home() {
    let parent = ParentEnv::from_vars(vars(&[("USER", "u"), ("LOGNAME", "u"), ("TMPDIR", "/t")]));
    assert_eq!(parent.child_env(), Err(EnvError::Missing { name: "HOME" }));
}

#[test]
fn child_env_refuses_a_missing_user_or_logname() {
    let parent = ParentEnv::from_vars(vars(&[("HOME", "/h"), ("LOGNAME", "u"), ("TMPDIR", "/t")]));
    assert_eq!(parent.child_env(), Err(EnvError::Missing { name: "USER" }));
    let parent = ParentEnv::from_vars(vars(&[("HOME", "/h"), ("USER", "u"), ("TMPDIR", "/t")]));
    assert_eq!(parent.child_env(), Err(EnvError::Missing { name: "LOGNAME" }));
}

#[test]
fn child_env_refuses_a_relative_tmpdir_or_home() {
    let parent = ParentEnv::from_vars(vars(&[
        ("HOME", "/h"),
        ("USER", "u"),
        ("LOGNAME", "u"),
        ("TMPDIR", "tmp"),
    ]));
    assert_eq!(
        parent.child_env(),
        Err(EnvError::NotAbsolute { name: "TMPDIR" })
    );
    let parent = ParentEnv::from_vars(vars(&[
        ("HOME", "h"),
        ("USER", "u"),
        ("LOGNAME", "u"),
        ("TMPDIR", "/t"),
    ]));
    assert_eq!(parent.child_env(), Err(EnvError::NotAbsolute { name: "HOME" }));
}

#[test]
fn parent_accessors_return_the_forwarded_paths() {
    let parent = ParentEnv::from_vars(vars(&FULL_PARENT));
    assert_eq!(parent.home(), Some(std::path::Path::new("/h")));
    assert_eq!(parent.tmpdir(), Some(std::path::Path::new("/t")));
    assert!(parent.refused().is_empty());
}

#[test]
fn version_parses_exactly_three_numbers() {
    assert_eq!(
        ClaudeVersion::parse("2.1.304"),
        Some(ClaudeVersion::new(2, 1, 304))
    );
    for text in [
        "2.1.304-beta",
        "v2.1.304",
        "2.1",
        "2.1.304.1",
        "",
        "2..304",
        "2.1.304 ",
        "1234567890.1.1",
        "02.1.304",
        "+2.1.304",
    ] {
        assert_eq!(ClaudeVersion::parse(text), None, "{text:?}");
    }
    assert_eq!(ClaudeVersion::new(2, 1, 304).to_string(), "2.1.304");
}

#[test]
fn version_output_has_one_form() {
    assert_eq!(
        parse_version_output(b"2.1.304 (Claude Code)\n"),
        Some(ClaudeVersion::new(2, 1, 304))
    );
    assert_eq!(
        parse_version_output(b"2.1.304 (Claude Code)"),
        Some(ClaudeVersion::new(2, 1, 304))
    );
    for stdout in [
        b"2.1.304 (Claude Code)\n\n".as_slice(),
        b"2.1.304 (Claude Code) \n",
        b"2.1.304 (Claude)\n",
        b"2.1.304\n",
        b"2.1.304 (Claude Code)\r\n",
        b"wrapper 2.1.304 (Claude Code)\n",
        b"",
        b"\xff (Claude Code)\n",
    ] {
        assert_eq!(parse_version_output(stdout), None, "{stdout:?}");
    }
}

#[test]
fn versions_order_numerically() {
    assert!(ClaudeVersion::new(2, 10, 0) > ClaudeVersion::new(2, 9, 9));
    assert!(ClaudeVersion::new(3, 0, 0) > ClaudeVersion::new(2, 99, 99));
    assert!(ClaudeVersion::new(2, 1, 304) > MIN_CLAUDE_VERSION);
    assert_eq!(MIN_CLAUDE_VERSION, ClaudeVersion::new(2, 1, 296));
    assert_eq!(
        ClaudeVersion::parse("2.10.0").map(|v| v > ClaudeVersion::new(2, 9, 9)),
        Some(true)
    );
}
