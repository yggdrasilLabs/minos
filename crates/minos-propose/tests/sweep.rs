//! The sweep tier (PROP-10, 06-CONTEXT D-05): proposals drawn from core's seeded sampler with no model call,
//! rendered against the parent tree, cumulative from the base, deterministic per run and generation, and dropped
//! by the same closed reasons as model proposals.

use std::error::Error;

use minos_core::diff::apply::{Tree, apply};
use minos_core::diff::cumulative::BaseTree;
use minos_core::id::{ManifestId, ModelId, RunId, TreeId};
use minos_core::ledger::Origin;
use minos_core::manifest::{Manifest, ParamName, ProposerSpec, Tier};
use minos_core::path::RelPath;
use minos_core::sweep::{ParamValue, SweepError, sample};
use minos_propose::prompt::History;
use minos_propose::proposal::DropReason;
use minos_propose::proposer::{Proposals, ProposeError, ProposeInput};
use minos_propose::sweep;

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest with two writable files and no params; tests append `[[param]]` tables after it. The
/// single-shot proposer keeps the manifest valid when it declares no params.
const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/fmt.rs", "src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

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
model = "haiku"
batch = 4
"#;

/// An integer range and a choice list, both anchored in `src/parse.rs`.
const PARAMS: &str = r#"
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

/// One choice param with two values, so a large batch both repeats a value and hits the parent's value.
const TWO_CHOICES: &str = r#"
[[param]]
name = "b"
file = "src/parse.rs"
anchor = "const B: &str = "
choices = ["x", "y"]
"#;

const PARSE_BASE: &str = "const A: i64 = 50\nconst B: &str = x\n";
const PARSE_B_Y: &str = "const A: i64 = 50\nconst B: &str = y\n";
const FMT_BASE: &str = "pub fn fmt(n: usize) -> String {\n    n.to_string()\n}\n";
const FMT_PARENT: &str = "pub fn fmt(n: usize) -> String {\n    format!(\"{n}\")\n}\n";
const LIB: &str = "pub mod fmt;\npub mod parse;\n";
const PROGRAM: &[u8] = b"Make parsing faster.\n";

fn manifest(params: &str) -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(&format!("{MANIFEST}{params}"))?)
}

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

fn tree(files: &[(&str, &str)]) -> Result<Tree, Box<dyn Error>> {
    let mut tree = Tree::new();
    for (name, content) in files {
        tree.insert(path(name)?, content.as_bytes().to_vec());
    }
    Ok(tree)
}

fn base() -> Result<Tree, Box<dyn Error>> {
    tree(&[
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ])
}

fn run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000)
}

fn sweep_spec(batch: u32) -> Result<ProposerSpec, Box<dyn Error>> {
    Ok(ProposerSpec::new(Tier::Sweep, None, batch)?)
}

/// Runs the sweep tier for `generation` of [`run`] against `base` and `parent`.
fn propose(
    manifest: &Manifest,
    spec: &ProposerSpec,
    generation: u32,
    base: &Tree,
    parent: &Tree,
) -> Result<Proposals, ProposeError> {
    let history = History::empty();
    let run = run();
    // A fixed tree ID: the tests compare diffs and trees, not IDs.
    let base = BaseTree::new(TreeId::of(b"base"), base.clone());
    let input = ProposeInput {
        manifest,
        run: &run,
        generation,
        base: &base,
        parent,
        program: PROGRAM,
        history: &history,
        notes: &[],
    };
    sweep::propose(spec, &input)
}

/// The samples the tier draws for `generation`, as `b`'s rendered value per sample.
fn b_values(
    manifest: &Manifest,
    generation: u32,
    batch: u32,
) -> Result<Vec<String>, Box<dyn Error>> {
    let name = "b".parse::<ParamName>()?;
    let samples = sample(&run(), generation, manifest.params(), batch)?;
    samples
        .iter()
        .map(|item| {
            item.get(&name)
                .map(ParamValue::render_text)
                .ok_or_else(|| "a sample has no value for b".into())
        })
        .collect()
}

fn count(values: &[String], value: &str) -> u32 {
    u32::try_from(values.iter().filter(|item| *item == value).count()).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn sweep_proposals_apply_to_the_base_with_sample_rationales() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let proposals = propose(&manifest, &sweep_spec(4)?, 1, &base, &base)?;
    let samples = sample(&run(), 1, manifest.params(), 4)?;
    let a = "a".parse::<ParamName>()?;
    let b = "b".parse::<ParamName>()?;

    assert_eq!(proposals.tier(), Tier::Sweep);
    assert_eq!(proposals.origin(), &Origin::Sweep);
    assert!(proposals.usage().is_none());
    assert!(proposals.models().is_empty());
    assert!(proposals.empty().is_none());
    assert_eq!(proposals.dropped().total(), 0);
    assert_eq!(proposals.items().len(), samples.len());
    for (proposal, drawn) in proposals.items().iter().zip(&samples) {
        assert_eq!(proposal.rationale(), drawn.describe());
        let applied = apply(&base, proposal.diff())?;
        let a_text = drawn.get(&a).map(ParamValue::render_text);
        let b_text = drawn.get(&b).map(ParamValue::render_text);
        let expected = format!(
            "const A: i64 = {}\nconst B: &str = {}\n",
            a_text.unwrap_or_default(),
            b_text.unwrap_or_default()
        );
        assert_eq!(
            applied.get(&path("src/parse.rs")?),
            Some(expected.as_bytes())
        );
        assert_eq!(applied.get(&path("src/fmt.rs")?), Some(FMT_BASE.as_bytes()));
    }
    Ok(())
}

#[test]
fn sweep_is_deterministic_per_run_and_generation() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let spec = sweep_spec(8)?;
    let first = propose(&manifest, &spec, 3, &base, &base)?;
    let second = propose(&manifest, &spec, 3, &base, &base)?;
    let first_bytes: Vec<&[u8]> = first.items().iter().map(|p| p.diff().as_bytes()).collect();
    let second_bytes: Vec<&[u8]> = second.items().iter().map(|p| p.diff().as_bytes()).collect();
    assert!(!first_bytes.is_empty());
    assert_eq!(first_bytes, second_bytes);
    assert_eq!(first, second);
    Ok(())
}

#[test]
fn sweep_diffs_are_cumulative_from_the_base() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let parent = tree(&[
        ("src/fmt.rs", FMT_PARENT),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ])?;
    let proposals = propose(&manifest, &sweep_spec(4)?, 1, &base, &parent)?;
    assert!(!proposals.items().is_empty());
    for proposal in proposals.items() {
        let applied = apply(&base, proposal.diff())?;
        assert_eq!(
            applied.get(&path("src/fmt.rs")?),
            Some(FMT_PARENT.as_bytes())
        );
        assert!(proposal.diff().paths().any(|p| p.as_str() == "src/fmt.rs"));
    }
    Ok(())
}

#[test]
fn a_sample_equal_to_the_parent_is_dropped_as_same_as_parent() -> TestResult {
    let manifest = manifest(TWO_CHOICES)?;
    let base = base()?;
    let values = b_values(&manifest, 1, 16)?;
    let same = count(&values, "x");
    assert!(same > 0, "the fixture must draw the parent's value");
    let proposals = propose(&manifest, &sweep_spec(16)?, 1, &base, &base)?;
    assert_eq!(proposals.dropped().get(DropReason::SameAsParent), same);
    Ok(())
}

#[test]
fn a_sample_that_restores_the_base_is_dropped_as_back_to_base() -> TestResult {
    let manifest = manifest(TWO_CHOICES)?;
    let base = base()?;
    let parent = tree(&[
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_B_Y),
    ])?;
    let values = b_values(&manifest, 1, 16)?;
    let back = count(&values, "x");
    let same = count(&values, "y");
    assert!(back > 0, "the fixture must draw the base's value");
    let proposals = propose(&manifest, &sweep_spec(16)?, 1, &base, &parent)?;
    assert_eq!(proposals.dropped().get(DropReason::BackToBase), back);
    assert_eq!(proposals.dropped().get(DropReason::SameAsParent), same);
    assert!(proposals.items().is_empty());
    Ok(())
}

#[test]
fn two_samples_with_the_same_tree_give_one_proposal() -> TestResult {
    let manifest = manifest(TWO_CHOICES)?;
    let base = base()?;
    let values = b_values(&manifest, 1, 16)?;
    let changed = count(&values, "y");
    assert!(changed > 1, "the fixture must draw the new value twice");
    let proposals = propose(&manifest, &sweep_spec(16)?, 1, &base, &base)?;
    assert_eq!(proposals.items().len(), 1);
    assert_eq!(
        proposals.dropped().get(DropReason::DuplicateInBatch),
        changed - 1
    );
    assert_eq!(proposals.dropped().total(), 16 - 1);
    Ok(())
}

#[test]
fn a_manifest_with_no_params_is_a_sweep_error() -> TestResult {
    let manifest = manifest("")?;
    let base = base()?;
    let result = propose(&manifest, &sweep_spec(4)?, 1, &base, &base);
    assert_eq!(result, Err(ProposeError::Sweep(SweepError::NoParams)));
    Ok(())
}

#[test]
fn generation_zero_is_refused() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let result = propose(&manifest, &sweep_spec(4)?, 0, &base, &base);
    assert_eq!(result, Err(ProposeError::Generation0));
    Ok(())
}

#[test]
fn base_and_parent_with_different_paths_are_refused() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let parent = tree(&[("src/fmt.rs", FMT_BASE), ("src/parse.rs", PARSE_BASE)])?;
    let result = propose(&manifest, &sweep_spec(4)?, 1, &base, &parent);
    assert_eq!(result, Err(ProposeError::TreeMismatch));
    Ok(())
}

#[test]
fn a_model_spec_is_a_tier_mismatch() -> TestResult {
    let manifest = manifest(PARAMS)?;
    let base = base()?;
    let spec = ProposerSpec::new(Tier::SingleShot, Some("haiku".parse::<ModelId>()?), 4)?;
    let result = propose(&manifest, &spec, 1, &base, &base);
    assert_eq!(
        result,
        Err(ProposeError::TierMismatch {
            expected: Tier::Sweep,
            found: Tier::SingleShot,
        })
    );
    Ok(())
}
