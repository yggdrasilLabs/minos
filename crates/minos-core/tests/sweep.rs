//! The seeded sweep (CORE-12): the seed, the raw ChaCha8 stream against a published known-answer vector, golden
//! samples, determinism, and the range of every drawn value.
//!
//! Params are built by parsing a manifest through `Manifest::parse_toml`, so every param here is one a user could
//! declare.

use std::error::Error;

use minos_core::id::{ManifestId, RunId, TreeId};
use minos_core::manifest::{Manifest, ParamDomain, ParamName, SweepParam};
use minos_core::sweep::{ParamValue, Sample, SweepError, sample, seed};
use proptest::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

type TestResult = Result<(), Box<dyn Error>>;

/// A valid manifest with two writable files and no params; tests append `[[param]]` tables after it.
const BASE_MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/conf.rs", "src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// The golden params: `a` over 0, 5, ..., 100 and `b` over three choices, in that declaration order.
const GOLDEN_PARAMS: &str = r#"
[[param]]
name = "a"
file = "src/parse.rs"
anchor = "const A: i64 = "
min = 0
max = 100
step = 5

[[param]]
name = "b"
file = "src/parse.rs"
anchor = "const B: &str = "
choices = ["x", "y", "z"]
"#;

/// The base manifest with `params` appended.
fn manifest(params: &str) -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(&format!("{BASE_MANIFEST}{params}"))?)
}

/// The run ID of `tests/golden_ids.rs` (`run_id_golden_vector`).
fn golden_run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
}

fn name(text: &str) -> Result<ParamName, Box<dyn Error>> {
    Ok(text.parse::<ParamName>()?)
}

/// Whether `value` is a value `param` may take.
fn in_domain(param: &SweepParam, value: &ParamValue) -> bool {
    match (param.domain(), value) {
        (ParamDomain::Int(range), ParamValue::Int(value)) => {
            let offset = i128::from(*value) - i128::from(range.min());
            *value >= range.min()
                && *value <= range.max()
                && offset % i128::from(range.step()) == 0
        }
        (ParamDomain::Choices(choices), ParamValue::Choice(value)) => {
            choices.as_slice().iter().any(|choice| choice == value)
        }
        _ => false,
    }
}

/// Every param is assigned exactly once, every value is in its domain, and no other name appears.
fn well_formed(params: &[SweepParam], sample: &Sample) -> bool {
    sample.iter().count() == params.len()
        && params.iter().all(|param| {
            sample
                .get(param.name())
                .is_some_and(|value| in_domain(param, value))
        })
}

// ---------------------------------------------------------------------------------------------------------------
// The raw stream
// ---------------------------------------------------------------------------------------------------------------

/// The ChaCha8 stream the sweep draws from is the published cipher, checked against a vector that does not come
/// from the code under test.
///
/// Source: https://www.ietf.org/archive/id/draft-strombergson-chacha-test-vectors-01.txt, test case TC1
/// (all-zero 256-bit key, all-zero IV), "Rounds: 8". Its first 32 keystream bytes are
///
/// ```text
/// 3e 00 ef 2f 89 5f 40 d6  7f 5b b8 e8 1f 09 a5 a1  2c 84 0e c3 ce 9a 7f 3b  18 1b e1 88 ef 71 1a 1e
/// ```
///
/// `next_u32` reads four bytes little-endian (`3e00ef2f` -> 0x2fef003e), and `next_u64` reads two such words,
/// low word first (0x2fef003e, then 0xd6405f89 -> 0xd6405f892fef003e). If this test fails, the literals must not
/// be edited: the sweep's stream would not be the published cipher.
#[test]
fn chacha8_known_answer() {
    let mut rng = ChaCha8Rng::from_seed([0_u8; 32]);
    let words = [rng.next_u64(), rng.next_u64(), rng.next_u64(), rng.next_u64()];
    assert_eq!(
        words,
        [
            0xd640_5f89_2fef_003e,
            0xa1a5_091f_e8b8_5b7f,
            0x3b7f_9ace_c30e_842c,
            0x1e1a_71ef_88e1_1b18,
        ]
    );
    // The same 32 bytes as u32 words, which separates a word-order change in next_u64 from a different stream.
    let mut rng = ChaCha8Rng::from_seed([0_u8; 32]);
    let halves: Vec<u32> = (0..8).map(|_| rng.next_u32()).collect();
    assert_eq!(
        halves,
        [
            0x2fef_003e,
            0xd640_5f89,
            0xe8b8_5b7f,
            0xa1a5_091f,
            0xc30e_842c,
            0x3b7f_9ace,
            0x88e1_1b18,
            0x1e1a_71ef,
        ]
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Seed
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn seed_differs_by_generation_and_run() {
    let run = golden_run();
    let other = RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_001);
    assert_ne!(seed(&run, 1), seed(&run, 2));
    assert_ne!(seed(&run, 1), seed(&other, 1));
    assert_eq!(seed(&run, 1), seed(&run, 1));
}

#[test]
fn seed_golden_vector() {
    // RUN=6a65b71b41e24c3b7d21e317e6a8fc29a41fcd29085ec93bbe13aa9b464c56cb (run_id_golden_vector)
    // { printf 'minos/sweep/v1\0'; printf '%s' "$RUN" | xxd -r -p; printf '%016x' 1 | xxd -r -p; } | shasum -a 256
    let digest = seed(&golden_run(), 1);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(hex, SEED_GOLDEN_HEX);
}

/// Recorded with the shell command in [`seed_golden_vector`], independently of the code under test.
const SEED_GOLDEN_HEX: &str = "5ccbb1c0dc095fb5daf738cbd9936ab5e3cf51e466779df9d51d897a23ce70cf";

// ---------------------------------------------------------------------------------------------------------------
// Sample
// ---------------------------------------------------------------------------------------------------------------

/// Pins the sweep's value stream: the run of `golden_ids.rs`, generation 1, the golden params, batch 4.
///
/// These literals were recorded from the first run of this test. They rest on [`chacha8_known_answer`] and
/// [`seed_golden_vector`], and they change only with a deliberate decision, such as a new seed domain.
#[test]
fn sweep_golden_samples() -> TestResult {
    let manifest = manifest(GOLDEN_PARAMS)?;
    let samples = sample(&golden_run(), 1, manifest.params(), 4)?;
    let described: Vec<String> = samples.iter().map(Sample::describe).collect();
    assert_eq!(described, GOLDEN_SAMPLES);
    Ok(())
}

/// The golden batch, in draw order.
const GOLDEN_SAMPLES: [&str; 4] = ["GOLDEN", "GOLDEN", "GOLDEN", "GOLDEN"];

#[test]
fn sample_values_are_typed() -> TestResult {
    let manifest = manifest(GOLDEN_PARAMS)?;
    let samples = sample(&golden_run(), 1, manifest.params(), 4)?;
    for item in &samples {
        assert!(matches!(item.get(&name("a")?), Some(ParamValue::Int(_))));
        assert!(matches!(item.get(&name("b")?), Some(ParamValue::Choice(_))));
        assert!(item.get(&name("c")?).is_none());
    }
    Ok(())
}

#[test]
fn render_text_is_decimal_or_the_choice() {
    assert_eq!(ParamValue::Int(-42).render_text(), "-42");
    assert_eq!(ParamValue::Int(16).render_text(), "16");
    assert_eq!(ParamValue::Choice("fast".to_owned()).render_text(), "fast");
}

#[test]
fn sample_returns_the_batch_size() -> TestResult {
    let manifest = manifest(GOLDEN_PARAMS)?;
    for batch in [0, 1, 7] {
        let samples = sample(&golden_run(), 3, manifest.params(), batch)?;
        assert_eq!(samples.len(), usize::try_from(batch)?);
        assert!(samples.iter().all(|item| well_formed(manifest.params(), item)));
    }
    Ok(())
}

#[test]
fn sample_with_no_params_is_refused() {
    assert!(matches!(
        sample(&golden_run(), 1, &[], 4),
        Err(SweepError::NoParams)
    ));
}

#[test]
fn single_point_domains_always_give_that_point() -> TestResult {
    let manifest = manifest(
        r#"
[[param]]
name = "only"
file = "src/conf.rs"
anchor = "const ONLY: i64 = "
min = 7
max = 7
step = 1

[[param]]
name = "pick"
file = "src/conf.rs"
anchor = "const PICK: &str = "
choices = ["one"]
"#,
    )?;
    for item in sample(&golden_run(), 9, manifest.params(), 5)? {
        assert_eq!(item.describe(), "only=7, pick=one");
    }
    Ok(())
}

proptest! {
    #[test]
    fn identical_inputs_identical_batches(
        manifest_bytes in proptest::collection::vec(any::<u8>(), 0..32),
        started_at in any::<u64>(),
        generation in any::<u32>(),
        batch in 0_u32..16,
    ) {
        let manifest = manifest(GOLDEN_PARAMS).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let run = RunId::of(&ManifestId::of(&manifest_bytes), &TreeId::of(b"t"), started_at);
        let first = sample(&run, generation, manifest.params(), batch);
        let second = sample(&run, generation, manifest.params(), batch);
        prop_assert_eq!(first, second);
    }

    #[test]
    fn every_value_is_in_its_domain(
        started_at in any::<u64>(),
        generation in any::<u32>(),
        batch in 1_u32..16,
    ) {
        let manifest = manifest(GOLDEN_PARAMS).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let run = RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), started_at);
        let samples = sample(&run, generation, manifest.params(), batch)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let expected = usize::try_from(batch).map_err(|error| TestCaseError::fail(error.to_string()))?;
        prop_assert_eq!(samples.len(), expected);
        prop_assert!(samples.iter().all(|item| well_formed(manifest.params(), item)));
    }
}
