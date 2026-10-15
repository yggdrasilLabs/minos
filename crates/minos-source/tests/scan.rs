//! The secret scan: a hit and a near miss for every pattern, location-only reports, and a self-scan of this
//! crate's own sources.
//!
//! No complete secret-shaped value appears in this file. Every hit vector is joined at run time from pieces that
//! do not match on their own (04-RESEARCH Pitfall 5), so Minos's own tree passes its own scan and a forge's push
//! protection has nothing to block.

use std::error::Error;
use std::fs;
use std::path::Path;

use minos_core::path::RelPath;
use minos_source::scan::PATTERN_NAMES;
use minos_source::{Scanner, SecretHit};

type TestResult = Result<(), Box<dyn Error>>;

/// `n` characters cycling through `[A-Za-z0-9]`.
fn alnum(n: usize) -> String {
    const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    ALPHABET.chars().cycle().take(n).collect()
}

/// `n` characters cycling through `[A-Z]`.
fn upper(n: usize) -> String {
    ('A'..='Z').cycle().take(n).collect()
}

/// One hit vector per pattern, in `PATTERN_NAMES` order, each assembled at run time.
fn hit_vectors() -> [(&'static str, String); 9] {
    [
        (
            "pem-private-key",
            format!("{}{}{}", "-----BEGIN ", "RSA PRIVATE", " KEY-----"),
        ),
        ("aws-access-key-id", format!("{}{}{}", "AK", "IA", upper(16))),
        ("github-token", format!("{}{}{}", "gh", "p_", alnum(36))),
        (
            "github-fine-grained-pat",
            format!("{}{}{}", "github", "_pat_", alnum(82)),
        ),
        ("anthropic-api-key", format!("{}{}{}", "sk-", "ant-", alnum(30))),
        (
            "openai-api-key",
            format!(
                "{}{}{}{}{}",
                "sk-proj-",
                alnum(20),
                "T3Blbk",
                "FJ",
                alnum(20)
            ),
        ),
        ("slack-token", format!("{}{}{}", "xo", "xb-", alnum(12))),
        (
            "jwt",
            format!(
                "{e}{a}.{e}{b}.{c}",
                e = format!("{}{}", "ey", "J"),
                a = alnum(12),
                b = alnum(14),
                c = alnum(16)
            ),
        ),
        ("google-api-key", format!("{}{}{}", "AI", "za", alnum(35))),
    ]
}

fn rel(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

#[test]
fn hit_vectors_follow_pattern_names_order() {
    let names: Vec<&str> = hit_vectors().iter().map(|(name, _)| *name).collect();
    assert_eq!(names, PATTERN_NAMES);
}

#[test]
fn every_pattern_hits_its_vector_on_the_right_line() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("src/config.rs")?;
    for (name, vector) in hit_vectors() {
        let text = format!("first line\nsecond line\nlet value = \"{vector}\";\nlast\n");
        let hits = scanner.scan(&path, text.as_bytes());
        assert_eq!(hits.len(), 1, "{name}: {hits:?}");
        let hit = hits.first().ok_or("no hit")?;
        assert_eq!(hit.pattern(), name);
        assert_eq!(hit.line(), 3, "{name}");
        assert_eq!(hit.path(), &path);
    }
    Ok(())
}

#[test]
fn every_pattern_hits_inside_binary_bytes() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("assets/blob.bin")?;
    for (name, vector) in hit_vectors() {
        let mut bytes = vec![0xff, 0x00, 0xfe];
        bytes.extend_from_slice(vector.as_bytes());
        bytes.extend_from_slice(&[0x00, 0xff]);
        let hits = scanner.scan(&path, &bytes);
        assert_eq!(hits.len(), 1, "{name}: {hits:?}");
        let hit = hits.first().ok_or("no hit")?;
        assert_eq!(hit.pattern(), name);
        assert_eq!(hit.line(), 1, "{name}");
    }
    Ok(())
}

#[test]
fn near_misses_do_not_hit() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("src/near.rs")?;
    let near_misses = [
        format!("{}{}", "AKIA", "1234"),
        "-----BEGIN PUBLIC KEY-----".to_owned(),
        "0123456789abcdef".repeat(4),
        format!("{}{}", "ghp_", alnum(35)),
        format!("{}{}", "sk-ant-", alnum(10)),
        format!("{}{}", "xoxz-", alnum(12)),
        format!("{}{}.{}{}", "eyJ", alnum(12), "eyJ", alnum(12)),
        format!("{}{}", "AIza", alnum(34)),
        format!("{}{}", "sk-", alnum(40)),
    ];
    for text in near_misses {
        let hits = scanner.scan(&path, text.as_bytes());
        assert!(hits.is_empty(), "{text}: {hits:?}");
    }
    Ok(())
}

#[test]
fn hits_are_sorted_by_line_then_pattern() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("src/many.rs")?;
    let [_, (_, aws), (_, github), _, _, _, _, _, (_, google)] = hit_vectors();
    let text = format!("one\n{google} {aws}\nthree\nfour\nfive\nsix\n{github}\n");
    let hits = scanner.scan(&path, text.as_bytes());
    let found: Vec<(u64, &str)> = hits.iter().map(|hit| (hit.line(), hit.pattern())).collect();
    assert_eq!(
        found,
        [
            (2, "aws-access-key-id"),
            (2, "google-api-key"),
            (7, "github-token")
        ]
    );
    Ok(())
}

#[test]
fn repeated_hits_of_one_pattern_are_all_reported() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("src/twice.rs")?;
    let [_, _, (_, github), ..] = hit_vectors();
    let text = format!("{github}\n\n{github} {github}\n");
    let lines: Vec<u64> = scanner
        .scan(&path, text.as_bytes())
        .iter()
        .map(SecretHit::line)
        .collect();
    assert_eq!(lines, [1, 3, 3]);
    Ok(())
}

#[test]
fn a_hit_names_path_pattern_and_line_but_never_the_bytes() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("deploy/keys.txt")?;
    let [_, (_, aws), ..] = hit_vectors();
    let text = format!("x\n\n\n\n{aws}\n");
    let hits = scanner.scan(&path, text.as_bytes());
    let hit = hits.first().ok_or("no hit")?;
    let shown = hit.to_string();
    assert_eq!(shown, "deploy/keys.txt:5: matches aws-access-key-id");
    assert!(!shown.contains(&aws));
    assert!(!shown.contains(&upper(16)));
    assert!(!format!("{hit:?}").contains(&upper(16)));
    Ok(())
}

#[test]
fn empty_and_clean_inputs_have_no_hits() -> TestResult {
    let scanner = Scanner::new()?;
    let path = rel("src/lib.rs")?;
    assert!(scanner.scan(&path, b"").is_empty());
    assert!(scanner.scan(&path, b"fn main() {}\n").is_empty());
    Ok(())
}

/// Scans every `.rs` file directly under `dir`, returning the number of files scanned.
fn scan_dir(scanner: &Scanner, dir: &Path) -> Result<usize, Box<dyn Error>> {
    let mut scanned = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("non-UTF-8 file name")?;
        if entry.file_type()?.is_file() && name.ends_with(".rs") {
            let hits = scanner.scan(&rel(name)?, &fs::read(entry.path())?);
            assert!(hits.is_empty(), "{}: {hits:?}", entry.path().display());
            scanned += 1;
        }
    }
    Ok(scanned)
}

#[test]
fn own_sources_have_no_secrets() -> TestResult {
    let scanner = Scanner::new()?;
    let sources = scan_dir(
        &scanner,
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
    )?;
    let tests = scan_dir(
        &scanner,
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests")),
    )?;
    assert!(sources >= 3, "{sources} source files scanned");
    assert!(tests >= 2, "{tests} test files scanned");
    Ok(())
}
