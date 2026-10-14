//! Golden vectors for every hash in `minos_core::id`, and strict parsing of IDs and names.
//!
//! Every hex literal below was computed outside the code under test with `/usr/bin/shasum -a 256` and `xxd`; the
//! shell recipe sits in a comment above each literal so a reviewer can recompute it. These are plain `assert_eq!`
//! checks on hex literals and never snapshot assertions, so they cannot be auto-accepted.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{
    BlobId, ExperimentId, IdHasher, IdParseError, ImageDigest, ManifestId, MetricName, ModelId,
    NameError, PricesId, PromptHash, ReportHash, RunId, TreeId,
};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

// printf '' | shasum -a 256
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
// printf 'abc' | shasum -a 256
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
#[should_panic(expected = "an IdHasher domain contains no NUL byte")]
fn id_hasher_refuses_a_nul_in_the_domain() {
    // The NUL after the domain is its terminator; a NUL inside would let "a" + "\0b" collide with "a\0b".
    let _ = IdHasher::new("minos/test\0v1");
}

#[test]
fn blob_id_is_the_plain_sha256() {
    assert_eq!(BlobId::of(b"").to_hex(), EMPTY_SHA256);
    assert_eq!(BlobId::of(b"abc").to_hex(), ABC_SHA256);
}

#[test]
fn plain_ids_equal_the_blob_digest() {
    for content in [&b""[..], &b"abc"[..]] {
        let blob = BlobId::of(content).to_hex();
        assert_eq!(TreeId::of(content).to_hex(), blob);
        assert_eq!(ManifestId::of(content).to_hex(), blob);
        assert_eq!(PricesId::of(content).to_hex(), blob);
        assert_eq!(ImageDigest::of(content).to_hex(), blob);
        assert_eq!(TreeId::of(content).to_blob_id(), BlobId::of(content));
    }
}

#[test]
fn baseline_golden_vector() {
    // EMPTY=$(printf '' | shasum -a 256 | cut -d' ' -f1)
    // { printf 'minos/experiment/v1\0'; printf '%s' "$EMPTY" | xxd -r -p; printf '\0\0\0\0\0\0\0\0'; } | shasum -a 256
    assert_eq!(
        ExperimentId::baseline(&TreeId::of(b"")).to_hex(),
        "5b161f655168a2b16645e03b77d61b0bfba4935d1be19c6293ec41a5998b6f86"
    );
    // T=$(printf 't' | shasum -a 256 | cut -d' ' -f1)
    // { printf 'minos/experiment/v1\0'; printf '%s' "$T" | xxd -r -p; printf '\0\0\0\0\0\0\0\0'; } | shasum -a 256
    assert_eq!(
        ExperimentId::baseline(&TreeId::of(b"t")).to_hex(),
        "6fd8be380eff4cb315d50a3f594a1baa7241164600eb22e68978748be04a75cb"
    );
}

#[test]
fn run_id_golden_vector() {
    // M=$(printf 'm' | shasum -a 256 | cut -d' ' -f1); T=$(printf 't' | shasum -a 256 | cut -d' ' -f1)
    // { printf 'minos/run/v1\0'; printf '%s' "$M" | xxd -r -p; printf '%s' "$T" | xxd -r -p;
    //   printf '%016x' 1760000000 | xxd -r -p; } | shasum -a 256
    let run = RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000);
    assert_eq!(
        run.to_hex(),
        "6a65b71b41e24c3b7d21e317e6a8fc29a41fcd29085ec93bbe13aa9b464c56cb"
    );
}

#[test]
fn prompt_hash_golden_vector() {
    // { printf 'minos/prompt/v1\0'; printf '%016x' 6 | xxd -r -p; printf 'prompt'; } | shasum -a 256
    assert_eq!(
        PromptHash::of(b"prompt").to_hex(),
        "c91644f81dc5d1904f6454527a0813431735367232d364c7408c4ff85566cf9b"
    );
}

#[test]
fn report_hash_golden_vector() {
    // { printf 'minos/report/v1\0'; printf '%016x' 6 | xxd -r -p; printf 'report'; } | shasum -a 256
    assert_eq!(
        ReportHash::of(b"report").to_hex(),
        "b3c176821e180f7bb71d623ca9a3666f282021764f3fa037fe11dc72c4b68487"
    );
}

#[test]
fn id_hasher_golden_vector() {
    // { printf 'minos/test/v1\0'; head -c 32 /dev/zero; printf '%016x' 7 | xxd -r -p;
    //   printf '%016x' 2 | xxd -r -p; printf 'xy'; } | shasum -a 256
    let digest = IdHasher::new("minos/test/v1")
        .digest(&[0_u8; 32])
        .u64(7)
        .bytes(b"xy")
        .finish();
    assert_eq!(
        base16ct::lower::encode_string(&digest),
        "014b1dd07237a54283bf46c7d274fdfab928326a8ad64348b781e48be119476e"
    );
}

#[test]
fn id_display_and_debug() {
    let id = BlobId::of(b"abc");
    assert_eq!(id.to_string(), ABC_SHA256);
    assert_eq!(format!("{id:?}"), format!("BlobId({ABC_SHA256})"));
    assert_eq!(base16ct::lower::encode_string(id.as_bytes()), ABC_SHA256);
}

#[test]
fn id_parse_is_strict() -> TestResult {
    let parsed = BlobId::from_str(ABC_SHA256)?;
    assert_eq!(parsed, BlobId::of(b"abc"));

    let upper = ABC_SHA256.to_uppercase();
    assert!(matches!(
        BlobId::from_str(&upper),
        Err(IdParseError::Malformed)
    ));
    let short = ABC_SHA256.get(..63).ok_or("63-character prefix")?;
    assert!(matches!(
        BlobId::from_str(short),
        Err(IdParseError::Malformed)
    ));
    let long = format!("{ABC_SHA256}0");
    assert!(matches!(
        BlobId::from_str(&long),
        Err(IdParseError::Malformed)
    ));
    let non_hex = format!("g{}", ABC_SHA256.get(1..).ok_or("suffix")?);
    assert!(matches!(
        BlobId::from_str(&non_hex),
        Err(IdParseError::Malformed)
    ));
    assert!(matches!(BlobId::from_str(""), Err(IdParseError::Malformed)));
    Ok(())
}

#[test]
fn id_json_is_a_lowercase_hex_string() -> TestResult {
    let id = ExperimentId::baseline(&TreeId::of(b""));
    let json = serde_json::to_string(&id)?;
    assert_eq!(
        json,
        "\"5b161f655168a2b16645e03b77d61b0bfba4935d1be19c6293ec41a5998b6f86\""
    );
    let back: ExperimentId = serde_json::from_str(&json)?;
    assert_eq!(back, id);

    assert!(serde_json::from_str::<BlobId>("1").is_err());
    assert!(serde_json::from_str::<BlobId>(&format!("\"{}\"", ABC_SHA256.to_uppercase())).is_err());
    assert!(serde_json::from_str::<BlobId>("\"abc\"").is_err());
    Ok(())
}

#[test]
fn metric_name_grammar() -> TestResult {
    let name = MetricName::from_str("throughput_mb_s")?;
    assert_eq!(name.as_str(), "throughput_mb_s");
    assert_eq!(name.to_string(), "throughput_mb_s");

    assert!(matches!(
        MetricName::from_str("Throughput"),
        Err(NameError::BadStart)
    ));
    assert!(matches!(
        MetricName::from_str("1abc"),
        Err(NameError::BadStart)
    ));
    assert!(matches!(MetricName::from_str(""), Err(NameError::Empty)));
    assert!(matches!(
        MetricName::from_str(&"a".repeat(65)),
        Err(NameError::TooLong { max: 64 })
    ));
    assert!(MetricName::from_str(&"a".repeat(64)).is_ok());
    assert!(matches!(
        MetricName::from_str("a\tb"),
        Err(NameError::BadByte { index: 1 })
    ));
    assert!(matches!(
        MetricName::from_str("ab\u{7f}"),
        Err(NameError::BadByte { index: 2 })
    ));
    assert!(matches!(
        MetricName::from_str("aB"),
        Err(NameError::BadByte { index: 1 })
    ));
    Ok(())
}

#[test]
fn metric_name_serde_runs_the_same_rules() -> TestResult {
    let name: MetricName = serde_json::from_str("\"identical_output\"")?;
    assert_eq!(serde_json::to_string(&name)?, "\"identical_output\"");
    assert!(serde_json::from_str::<MetricName>("\"Bad\"").is_err());
    assert!(serde_json::from_str::<MetricName>("\"\"").is_err());
    Ok(())
}

#[test]
fn model_id_grammar() -> TestResult {
    assert_eq!(ModelId::from_str("haiku")?.as_str(), "haiku");
    assert!(ModelId::from_str("us.anthropic.claude-x-v1:0").is_ok());
    assert!(ModelId::from_str(&"m".repeat(128)).is_ok());

    assert!(matches!(ModelId::from_str(""), Err(NameError::Empty)));
    assert!(matches!(
        ModelId::from_str(".haiku"),
        Err(NameError::BadStart)
    ));
    assert!(matches!(
        ModelId::from_str("a b"),
        Err(NameError::BadByte { index: 1 })
    ));
    assert!(matches!(
        ModelId::from_str(&"m".repeat(129)),
        Err(NameError::TooLong { max: 128 })
    ));
    assert!(serde_json::from_str::<ModelId>("\"a/b\"").is_err());
    Ok(())
}

proptest! {
    #[test]
    fn run_id_changes_with_every_field(
        manifest in proptest::collection::vec(any::<u8>(), 0..64),
        other_manifest in proptest::collection::vec(any::<u8>(), 0..64),
        tree in proptest::collection::vec(any::<u8>(), 0..64),
        other_tree in proptest::collection::vec(any::<u8>(), 0..64),
        started_at in any::<u64>(),
        other_started_at in any::<u64>(),
    ) {
        let m = ManifestId::of(&manifest);
        let t = TreeId::of(&tree);
        let run = RunId::of(&m, &t, started_at);
        prop_assert_eq!(run, RunId::of(&m, &t, started_at));
        if manifest != other_manifest {
            prop_assert_ne!(run, RunId::of(&ManifestId::of(&other_manifest), &t, started_at));
        }
        if tree != other_tree {
            prop_assert_ne!(run, RunId::of(&m, &TreeId::of(&other_tree), started_at));
        }
        if started_at != other_started_at {
            prop_assert_ne!(run, RunId::of(&m, &t, other_started_at));
        }
    }

    #[test]
    fn baseline_differs_per_tree(
        a in proptest::collection::vec(any::<u8>(), 0..64),
        b in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let left = ExperimentId::baseline(&TreeId::of(&a));
        let right = ExperimentId::baseline(&TreeId::of(&b));
        prop_assert_eq!(a == b, left == right);
    }

    #[test]
    fn id_hex_round_trips(content in proptest::collection::vec(any::<u8>(), 0..256)) {
        let id = BlobId::of(&content);
        let hex = id.to_hex();
        prop_assert_eq!(hex.len(), 64);
        prop_assert!(hex.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')));
        let parsed = BlobId::from_str(&hex);
        prop_assert_eq!(parsed.ok(), Some(id));
        let json = serde_json::to_string(&id);
        prop_assert!(json.is_ok());
        let back = json.ok().and_then(|text| serde_json::from_str::<BlobId>(&text).ok());
        prop_assert_eq!(back, Some(id));
    }
}
