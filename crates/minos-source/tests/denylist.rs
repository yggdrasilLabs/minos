//! The hard denylist: every D-07 name rule, matched ASCII case-insensitively, and the look-alikes it must not
//! catch.

use minos_source::denylist::DENYLIST;
use minos_source::{DenyRule, deny_component};

#[test]
fn every_rule_matches_its_names_in_any_ascii_case() {
    let cases: [(&str, DenyRule); 16] = [
        (".git", DenyRule::GitDir),
        (".GIT", DenyRule::GitDir),
        (".Minos", DenyRule::MinosDir),
        (".claude", DenyRule::ClaudeDir),
        (".aws", DenyRule::AwsDir),
        (".ssh", DenyRule::SshDir),
        (".env", DenyRule::EnvFile),
        (".env.local", DenyRule::EnvFile),
        (".envrc", DenyRule::EnvFile),
        (".ENV", DenyRule::EnvFile),
        ("cert.pem", DenyRule::PemFile),
        ("KEY.PEM", DenyRule::PemFile),
        ("id_rsa", DenyRule::SshKey),
        ("id_ed25519", DenyRule::SshKey),
        ("minos.lock", DenyRule::MinosLock),
        ("MINOS.LOCK", DenyRule::MinosLock),
    ];
    for (name, rule) in cases {
        assert_eq!(deny_component(name.as_bytes()), Some(rule), "{name}");
    }
}

#[test]
fn look_alike_names_are_not_denied() {
    for name in [
        ".github",
        ".gitignore",
        "environment.rs",
        "pem.rs",
        "ident.rs",
        "minos.toml",
        "ssh_config",
        "",
        "a",
    ] {
        assert_eq!(deny_component(name.as_bytes()), None, "{name}");
    }
}

#[test]
fn the_table_holds_the_nine_name_rules_each_with_a_pattern_and_a_reason() {
    assert_eq!(DENYLIST.len(), 9);
    assert!(!DENYLIST.contains(&DenyRule::JudgePath));
    for rule in DENYLIST.iter().copied().chain([DenyRule::JudgePath]) {
        assert!(!rule.pattern().is_empty(), "{rule:?}");
        assert!(!rule.reason().is_empty(), "{rule:?}");
    }
}

#[test]
fn every_listed_rule_matches_its_own_pattern_text() {
    // An exact or prefix pattern matches itself with the trailing `*` dropped; a suffix pattern matches a name
    // ending in it.
    for rule in DENYLIST {
        let pattern = rule.pattern();
        let name = pattern
            .strip_suffix('*')
            .map(str::to_owned)
            .or_else(|| pattern.strip_prefix('*').map(|tail| format!("x{tail}")))
            .unwrap_or_else(|| pattern.to_owned());
        assert_eq!(deny_component(name.as_bytes()), Some(rule), "{pattern}");
    }
}

#[test]
fn non_utf8_names_are_compared_as_bytes() {
    assert_eq!(deny_component(b"\xff.pem"), Some(DenyRule::PemFile));
    assert_eq!(deny_component(b"\xff.git"), None);
}
