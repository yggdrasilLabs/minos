//! Launch key golden vectors and properties, and the role tags they hash.
//!
//! Every hex literal below was computed outside the code under test with `/usr/bin/shasum -a 256` and `xxd`; the
//! shell recipe sits in a comment above each literal so a reviewer can recompute it. These are plain `assert_eq!`
//! checks on hex literals and never snapshot assertions, so they cannot be auto-accepted.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{ExperimentId, TreeId};
use minos_core::manifest::JudgeVersion;
use minos_sandbox::{Candidate, Judge, LaunchKey, Role, RoleKind};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// `ExperimentId::baseline(&TreeId::of(b"t"))`, pinned in `minos-core/tests/golden_ids.rs`.
const BASELINE_OF_T: &str = "6fd8be380eff4cb315d50a3f594a1baa7241164600eb22e68978748be04a75cb";

fn baseline_of_t() -> ExperimentId {
    ExperimentId::baseline(&TreeId::of(b"t"))
}

fn judge_ab() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"ab".repeat(32))?)
}

#[test]
fn launch_key_golden_vectors() -> TestResult {
    let exp = baseline_of_t();
    assert_eq!(exp.to_hex(), BASELINE_OF_T);
    let judge = judge_ab()?;
    // EXP=6fd8be380eff4cb315d50a3f594a1baa7241164600eb22e68978748be04a75cb; J=$(printf 'ab%.0s' $(seq 32))
    // { printf 'minos/launch/v1\0'; printf '%s' "$EXP" | xxd -r -p; printf '%016x' 1 | xxd -r -p;
    //   printf '%s' "$J" | xxd -r -p; printf '%016x' 0 | xxd -r -p; } | shasum -a 256
    assert_eq!(
        LaunchKey::of::<Candidate>(&exp, &judge, 0).to_hex(),
        "4747473e3894709a175a5f57f4406347fdc40c5b426b6fcb78ecb653c71534fb"
    );
    // { printf 'minos/launch/v1\0'; printf '%s' "$EXP" | xxd -r -p; printf '%016x' 2 | xxd -r -p;
    //   printf '%s' "$J" | xxd -r -p; printf '%016x' 0 | xxd -r -p; } | shasum -a 256
    assert_eq!(
        LaunchKey::of::<Judge>(&exp, &judge, 0).to_hex(),
        "a208611f3b16fd001a6ea2a726d520b2b7cea4c5ba27d75ca56fed2831169d05"
    );
    Ok(())
}

#[test]
fn role_tags_are_fixed() {
    assert_eq!(RoleKind::Candidate.tag(), 1);
    assert_eq!(RoleKind::Judge.tag(), 2);
    assert_eq!(Candidate::KIND, RoleKind::Candidate);
    assert_eq!(Judge::KIND, RoleKind::Judge);
}

#[test]
fn launch_key_prints_hex() -> TestResult {
    let key = LaunchKey::of::<Candidate>(&baseline_of_t(), &judge_ab()?, 0);
    let hex = key.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    );
    assert_eq!(key.to_string(), hex);
    assert!(format!("{key:?}").contains(&hex));
    assert_eq!(base16ct::lower::encode_string(key.as_bytes()), hex);
    Ok(())
}

/// A judge version over 32 arbitrary bytes, through its only public parser.
fn judge_of(bytes: &[u8; 32]) -> Result<JudgeVersion, TestCaseError> {
    JudgeVersion::from_str(&base16ct::lower::encode_string(bytes))
        .map_err(|error| TestCaseError::fail(error.to_string()))
}

proptest! {
    #[test]
    fn launch_key_sensitive_to_every_field(
        tree in proptest::collection::vec(any::<u8>(), 0..64),
        other_tree in proptest::collection::vec(any::<u8>(), 0..64),
        judge_bytes in any::<[u8; 32]>(),
        other_judge_bytes in any::<[u8; 32]>(),
        trial in any::<u32>(),
        other_trial in any::<u32>(),
    ) {
        let exp = ExperimentId::baseline(&TreeId::of(&tree));
        let judge = judge_of(&judge_bytes)?;
        let key = LaunchKey::of::<Candidate>(&exp, &judge, trial);
        prop_assert_eq!(key, LaunchKey::of::<Candidate>(&exp, &judge, trial));
        prop_assert_ne!(key, LaunchKey::of::<Judge>(&exp, &judge, trial));
        if tree != other_tree {
            let other_exp = ExperimentId::baseline(&TreeId::of(&other_tree));
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&other_exp, &judge, trial));
        }
        if judge_bytes != other_judge_bytes {
            let other_judge = judge_of(&other_judge_bytes)?;
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&exp, &other_judge, trial));
        }
        if trial != other_trial {
            prop_assert_ne!(key, LaunchKey::of::<Candidate>(&exp, &judge, other_trial));
        }
    }
}
