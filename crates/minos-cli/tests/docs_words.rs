//! The public documents never overclaim, and say what they must (DOCS-03, DOCS-04, and COST-02's docs half).
//!
//! Minos is public from its first commit, so what `README.md`, `SECURITY.md`, and the sample's
//! `examples/logparse/README.md` say is what users will believe the sandbox protects. This target reads the three
//! files from the repository and fails, naming the file and the item, when:
//!
//! - any of them contains `ironclad` (any case), the whole word `exact` (any case), or the phrase `cannot be
//!   escaped` (any case, across a line break). No isolation is perfect and Minos claims none; cost is always
//!   `approx.`, never "exact". Only the whole word is banned (08-RESEARCH Open Question 5, decided): `exactly` is
//!   ordinary English ("exactly one ID") and stays allowed. A word is a run of ASCII letters, digits, and `_`, so
//!   `exact-match` and `Exact.` are caught and `exactly` and `inexact` are not;
//! - `SECURITY.md` lacks a name of its trust base or its recorded exceptions: `libkrunfw`, `msb_krun`,
//!   `Hypervisor.framework`, `Seatbelt`, `_minos`, `KRUN_BOOT_START_NS`, `minos host doctor`, or `zero-day`;
//! - `README.md` lacks the provider-terms sentence's parts (`own install`, `API key`, `Bedrock`), the link to
//!   `SECURITY.md`, or the cost wording `approx. cost`.
//!
//! The required items are matched as written, case and all, because they are names.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The public documents, relative to the repository root.
const DOCS: [&str; 3] = ["README.md", "SECURITY.md", "examples/logparse/README.md"];

/// Words banned anywhere inside a word, compared on lowercased text.
const BANNED_SUBSTRINGS: [&str; 1] = ["ironclad"];

/// Words banned as whole words, compared on lowercased words.
const BANNED_WORDS: [&str; 1] = ["exact"];

/// Phrases banned on lowercased text with every run of whitespace collapsed to one space.
const BANNED_PHRASES: [&str; 1] = ["cannot be escaped"];

/// What `SECURITY.md` must name: the trust base, the recorded exceptions, the doctor, and the stated target.
const SECURITY_REQUIRED: [&str; 8] = [
    "libkrunfw",
    "msb_krun",
    "Hypervisor.framework",
    "Seatbelt",
    "_minos",
    "KRUN_BOOT_START_NS",
    "minos host doctor",
    "zero-day",
];

/// What `README.md` must say: the provider terms, the security link, and the cost wording.
const README_REQUIRED: [&str; 5] = [
    "own install",
    "API key",
    "Bedrock",
    "SECURITY.md",
    "approx. cost",
];

/// The repository root.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Reads `doc`, naming it when it cannot be read.
fn read_doc(doc: &str) -> TestResult<String> {
    fs::read_to_string(repo_root().join(doc))
        .map_err(|error| format!("{doc}: cannot be read: {error}").into())
}

/// Every banned item `text` contains, described for a failure message.
fn banned_in(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut found = Vec::new();
    for banned in BANNED_SUBSTRINGS {
        if lower.contains(banned) {
            found.push(format!("the word {banned:?}"));
        }
    }
    for banned in BANNED_WORDS {
        if lower
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|word| word == banned)
        {
            found.push(format!("the whole word {banned:?}"));
        }
    }
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    for banned in BANNED_PHRASES {
        if collapsed.contains(banned) {
            found.push(format!("the phrase {banned:?}"));
        }
    }
    found
}

/// Every item of `required` that `text` lacks.
fn missing_in<'a>(text: &str, required: &[&'a str]) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|item| !text.contains(item))
        .collect()
}

#[test]
fn the_checks_ban_the_whole_word_and_allow_exactly() {
    // Arrange
    let allowed = "It matches exactly one ID, and an inexact prefix is refused.";
    let cases = [
        "The figure is exact.",
        "An Exact-match check.",
        "EXACT",
        "The sandbox is IronClad.",
        "The sandbox cannot be\n  escaped.",
    ];

    // Act
    let allowed_found = banned_in(allowed);
    let cases_found: Vec<Vec<String>> = cases.iter().map(|case| banned_in(case)).collect();

    // Assert
    assert_eq!(allowed_found, Vec::<String>::new());
    for (case, found) in cases.iter().zip(&cases_found) {
        assert_eq!(found.len(), 1, "{case:?} gave {found:?}");
    }
    assert_eq!(
        missing_in(
            "libkrunfw and Seatbelt",
            &["libkrunfw", "Seatbelt", "msb_krun"]
        ),
        vec!["msb_krun"]
    );
}

#[test]
fn public_docs_hold_no_overclaim() -> TestResult {
    // Arrange
    let mut failures = Vec::new();

    // Act
    for doc in DOCS {
        let text = read_doc(doc)?;
        for item in banned_in(&text) {
            failures.push(format!("{doc} contains {item}"));
        }
    }

    // Assert
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn security_names_its_trust_base_and_exceptions() -> TestResult {
    // Arrange
    let text = read_doc("SECURITY.md")?;

    // Act
    let missing = missing_in(&text, &SECURITY_REQUIRED);

    // Assert
    assert!(missing.is_empty(), "SECURITY.md lacks {missing:?}");
    Ok(())
}

#[test]
fn readme_states_the_provider_terms_the_cost_wording_and_links_security() -> TestResult {
    // Arrange
    let text = read_doc("README.md")?;

    // Act
    let missing = missing_in(&text, &README_REQUIRED);

    // Assert
    assert!(missing.is_empty(), "README.md lacks {missing:?}");
    Ok(())
}
