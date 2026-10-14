//! `RelPath` grammar: accepted forms, each rejection with its exact `PathError`, bytewise ordering, serde through
//! the same validator, and property tests over the grammar and bytes outside it.

use std::error::Error;
use std::str::FromStr;

use minos_core::limits::{MAX_COMPONENT_BYTES, MAX_PATH_BYTES, MAX_PATH_DEPTH};
use minos_core::path::{PathError, RelPath};
use proptest::prelude::*;
use proptest::sample::Index;

type TestResult = Result<(), Box<dyn Error>>;

fn rejected(text: &str) -> Result<PathError, Box<dyn Error>> {
    match RelPath::from_str(text) {
        Ok(_) => Err(format!("{} bytes accepted", text.len()).into()),
        Err(error) => Ok(error),
    }
}

#[test]
fn accepts_plain_relative_paths() -> TestResult {
    for text in ["src/parse.rs", "a", ".gitignore", "dir/sub/file_1.rs", "a+b-c.txt"] {
        assert_eq!(RelPath::from_str(text)?.as_str(), text);
        assert_eq!(RelPath::try_from(text)?.as_str(), text);
        assert_eq!(RelPath::try_from(text.to_owned())?.to_string(), text);
    }
    Ok(())
}

#[test]
fn rejects_structural_forms() -> TestResult {
    assert!(matches!(rejected("")?, PathError::Empty));
    assert!(matches!(rejected("/abs")?, PathError::Absolute));
    assert!(matches!(rejected("/")?, PathError::Absolute));
    assert!(matches!(rejected("a/")?, PathError::TrailingSlash));
    assert!(matches!(rejected("a//b")?, PathError::EmptyComponent));
    assert!(matches!(rejected("./a")?, PathError::DotComponent));
    assert!(matches!(rejected("a/./b")?, PathError::DotComponent));
    assert!(matches!(rejected("a/.")?, PathError::DotComponent));
    assert!(matches!(rejected("../a")?, PathError::ParentComponent));
    assert!(matches!(rejected("a/..")?, PathError::ParentComponent));
    assert!(matches!(rejected("..")?, PathError::ParentComponent));
    assert!(matches!(rejected("-flag")?, PathError::LeadingDash));
    assert!(matches!(rejected("dir/-x")?, PathError::LeadingDash));
    Ok(())
}

#[test]
fn rejects_forbidden_bytes_with_their_index() -> TestResult {
    assert!(matches!(rejected("a\\b")?, PathError::ForbiddenByte { index: 1 }));
    assert!(matches!(rejected("a\u{0}b")?, PathError::ForbiddenByte { index: 1 }));
    // Cyrillic small a (U+0430) in place of the Latin a: a homoglyph of src/parse.rs.
    assert!(matches!(rejected("src/p\u{430}rse.rs")?, PathError::ForbiddenByte { index: 5 }));
    assert!(matches!(rejected("a b")?, PathError::ForbiddenByte { index: 1 }));
    assert!(matches!(rejected("a\nb")?, PathError::ForbiddenByte { index: 1 }));
    assert!(matches!(rejected("a:b")?, PathError::ForbiddenByte { index: 1 }));
    assert!(matches!(rejected("*")?, PathError::ForbiddenByte { index: 0 }));
    Ok(())
}

#[test]
fn rejects_over_length_and_depth() -> TestResult {
    let at_limit = format!("{}/{}", "a".repeat(255), "b".repeat(MAX_PATH_BYTES - 256));
    assert_eq!(at_limit.len(), MAX_PATH_BYTES);
    // Components of at most 255 bytes, so only the total length is over.
    let over = format!("{at_limit}c");
    assert!(matches!(rejected(&over)?, PathError::TooLong));

    let deep_ok = vec!["a"; MAX_PATH_DEPTH].join("/");
    assert_eq!(RelPath::from_str(&deep_ok)?.depth(), MAX_PATH_DEPTH);
    let too_deep = vec!["a"; MAX_PATH_DEPTH + 1].join("/");
    assert!(matches!(rejected(&too_deep)?, PathError::TooDeep));

    assert!(RelPath::from_str(&"c".repeat(MAX_COMPONENT_BYTES)).is_ok());
    let long_component = "c".repeat(MAX_COMPONENT_BYTES + 1);
    assert!(matches!(rejected(&long_component)?, PathError::ComponentTooLong));
    assert!(matches!(rejected(&format!("dir/{long_component}"))?, PathError::ComponentTooLong));
    Ok(())
}

#[test]
fn ordering_and_equality_are_bytewise() -> TestResult {
    let upper = RelPath::from_str("src/Parse.rs")?;
    let lower = RelPath::from_str("src/parse.rs")?;
    assert_ne!(upper, lower);
    assert!(upper < lower);
    assert_eq!(upper.ascii_case_key(), lower.ascii_case_key());
    assert_eq!(upper.ascii_case_key(), "src/parse.rs");
    Ok(())
}

#[test]
fn accessors() -> TestResult {
    let path = RelPath::from_str("dir/sub/file_1.rs")?;
    assert_eq!(path.components().collect::<Vec<_>>(), ["dir", "sub", "file_1.rs"]);
    assert_eq!(path.file_name(), "file_1.rs");
    assert_eq!(path.depth(), 3);
    let single = RelPath::from_str("a")?;
    assert_eq!(single.file_name(), "a");
    assert_eq!(single.depth(), 1);
    Ok(())
}

#[test]
fn is_within_respects_component_boundaries() -> TestResult {
    let judge = RelPath::from_str("judge")?;
    assert!(RelPath::from_str("judge/x")?.is_within(&judge));
    assert!(RelPath::from_str("judge/a/b")?.is_within(&judge));
    assert!(!RelPath::from_str("judgement")?.is_within(&judge));
    assert!(!RelPath::from_str("judge.py")?.is_within(&judge));
    assert!(!judge.is_within(&judge));
    assert!(!RelPath::from_str("src/judge/x")?.is_within(&judge));
    let nested = RelPath::from_str("a/b")?;
    assert!(RelPath::from_str("a/b/c")?.is_within(&nested));
    assert!(!RelPath::from_str("a/bc")?.is_within(&nested));
    Ok(())
}

#[test]
fn serde_runs_the_same_validator() -> TestResult {
    let path: RelPath = serde_json::from_str("\"src/parse.rs\"")?;
    assert_eq!(path.as_str(), "src/parse.rs");
    assert_eq!(serde_json::to_string(&path)?, "\"src/parse.rs\"");
    for bad in ["\"../x\"", "\"/abs\"", "\"\"", "\"a//b\"", "\"a\\\\b\"", "1"] {
        assert!(serde_json::from_str::<RelPath>(bad).is_err());
    }
    Ok(())
}

/// One component in the grammar: never `.` or `..`, never a leading dash, optionally a leading dot.
const COMPONENT: &str = "\\.?[A-Za-z0-9_+][A-Za-z0-9._+-]{0,14}";

fn grammar_path() -> impl Strategy<Value = String> {
    prop::collection::vec(COMPONENT, 1..8).prop_map(|components| components.join("/"))
}

fn outside_grammar() -> impl Strategy<Value = char> {
    any::<char>().prop_filter("a character outside the path grammar", |c| {
        !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-' | '/'))
    })
}

proptest! {
    #[test]
    fn grammar_strings_round_trip(text in grammar_path()) {
        let parsed = RelPath::from_str(&text);
        prop_assert!(parsed.is_ok());
        let path = parsed.map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(path.as_str(), text.as_str());
        prop_assert_eq!(path.depth(), path.components().count());
        let json = serde_json::to_string(&path).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let back: RelPath = serde_json::from_str(&json).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(back, path);
    }

    #[test]
    fn out_of_grammar_bytes_are_rejected(text in grammar_path(), bad in outside_grammar(), at in any::<Index>()) {
        let position = at.index(text.len() + 1);
        let (head, tail) = text.split_at_checked(position).ok_or_else(|| TestCaseError::fail("split"))?;
        let hostile = format!("{head}{bad}{tail}");
        match RelPath::from_str(&hostile) {
            Ok(_) => prop_assert!(false, "a path with a forbidden character was accepted"),
            Err(error) => prop_assert_eq!(error, PathError::ForbiddenByte { index: position }),
        }
    }

    #[test]
    fn ordering_is_bytewise(left in grammar_path(), right in grammar_path()) {
        let a = RelPath::from_str(&left).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let b = RelPath::from_str(&right).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(a.cmp(&b), left.as_bytes().cmp(right.as_bytes()));
        prop_assert_eq!(a == b, left == right);
    }
}
