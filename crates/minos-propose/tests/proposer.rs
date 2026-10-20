//! `Proposer<P>`, the loop's one entry point per generation (PROP-01, PROP-10): it dispatches on the spec's tier
//! with no trait object, never calls a model for a sweep, needs a provider only for a model tier, and refuses the
//! tiers that are not in this release.

use std::collections::BTreeMap;
use std::error::Error;

use minos_core::diff::apply::Tree;
use minos_core::id::{ManifestId, ModelId, RunId, TreeId};
use minos_core::manifest::{Manifest, ProposerSpec, Tier};
use minos_core::path::RelPath;
use minos_core::usage::Usage;
use minos_propose::{
    FakeProvider, FakeReply, History, Proposals, ProposeError, ProposeInput, Proposer,
};
use minos_propose::{single_shot, sweep};
use serde_json::{Map, Value};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

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
kind = "sweep"
batch = 4

[[proposer]]
kind = "single_shot"
model = "haiku"
batch = 4

[[param]]
name = "a"
file = "src/parse.rs"
anchor = "const A: i64 = "
min = 0
max = 100
step = 5
"#;

const PARSE_BASE: &str = "const A: i64 = 50\n";
const PARSE_NEW: &str = "const A: i64 = 55\n";
const FMT_BASE: &str = "pub fn fmt(n: usize) -> String {\n    n.to_string()\n}\n";
const LIB: &str = "pub mod fmt;\npub mod parse;\n";
const PROGRAM: &[u8] = b"Make parsing faster.\n";

fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?)
}

fn model(name: &str) -> Result<ModelId, Box<dyn Error>> {
    Ok(name.parse::<ModelId>()?)
}

fn base() -> Result<Tree, Box<dyn Error>> {
    let mut tree = Tree::new();
    for (name, content) in [
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ] {
        tree.insert(RelPath::try_from(name)?, content.as_bytes().to_vec());
    }
    Ok(tree)
}

fn sweep_spec() -> Result<ProposerSpec, Box<dyn Error>> {
    Ok(ProposerSpec::new(Tier::Sweep, None, 4)?)
}

fn single_shot_spec() -> Result<ProposerSpec, Box<dyn Error>> {
    Ok(ProposerSpec::new(
        Tier::SingleShot,
        Some(model("haiku")?),
        4,
    )?)
}

/// A script with one structured reply holding one valid proposal.
fn script() -> Vec<FakeReply> {
    let mut file = Map::new();
    file.insert("path".to_owned(), Value::from("src/parse.rs"));
    file.insert("content".to_owned(), Value::from(PARSE_NEW));
    let mut item = Map::new();
    item.insert("rationale".to_owned(), Value::from("raise A"));
    item.insert("files".to_owned(), Value::Array(vec![Value::Object(file)]));
    let mut output = Map::new();
    output.insert(
        "proposals".to_owned(),
        Value::Array(vec![Value::Object(item)]),
    );
    vec![FakeReply::Structured {
        output: Value::Object(output),
        usage: Usage::tokens(1, 2, 3, 4),
        models: BTreeMap::new(),
    }]
}

/// Calls `f` with the standard input for generation 1, base equal to parent.
fn with_input<T>(f: impl FnOnce(&ProposeInput<'_>) -> T) -> Result<T, Box<dyn Error>> {
    let manifest = manifest()?;
    let base = base()?;
    let history = History::empty();
    let run = RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000);
    let input = ProposeInput {
        manifest: &manifest,
        run: &run,
        generation: 1,
        base: &base,
        parent: &base,
        program: PROGRAM,
        history: &history,
        notes: &[],
    };
    Ok(f(&input))
}

// ---------------------------------------------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_sweep_spec_proposes_without_a_model_call() -> TestResult {
    let proposer = Proposer::with_provider(FakeProvider::new(script()));
    let spec = sweep_spec()?;
    let dispatched = with_input(|input| proposer.propose(&spec, input))??;
    let direct = with_input(|input| sweep::propose(&spec, input))??;
    assert!(!dispatched.items().is_empty());
    assert_eq!(dispatched, direct);
    let requests = proposer
        .provider()
        .ok_or("the proposer must hold its provider")?
        .requests()?;
    assert!(requests.is_empty());
    Ok(())
}

#[test]
fn a_single_shot_spec_calls_its_provider() -> TestResult {
    let proposer = Proposer::with_provider(FakeProvider::new(script()));
    let spec = single_shot_spec()?;
    let dispatched = with_input(|input| proposer.propose(&spec, input))??;
    let direct_fake = FakeProvider::new(script());
    let direct = with_input(|input| single_shot::propose(&direct_fake, &spec, input))??;
    assert_eq!(dispatched.items().len(), 1);
    assert_eq!(dispatched, direct);
    let requests = proposer
        .provider()
        .ok_or("the proposer must hold its provider")?
        .requests()?;
    assert_eq!(requests.len(), 1);
    Ok(())
}

#[test]
fn without_a_provider_only_the_sweep_proposes() -> TestResult {
    let proposer = Proposer::<FakeProvider>::without_provider();
    assert!(proposer.provider().is_none());
    let spec = single_shot_spec()?;
    let refused: Result<Proposals, ProposeError> =
        with_input(|input| proposer.propose(&spec, input))?;
    assert_eq!(refused, Err(ProposeError::NoProvider));
    let spec = sweep_spec()?;
    let swept = with_input(|input| proposer.propose(&spec, input))??;
    assert!(!swept.items().is_empty());
    Ok(())
}

#[test]
fn planned_and_agentic_are_not_in_this_release() -> TestResult {
    let proposer = Proposer::with_provider(FakeProvider::new(script()));
    for tier in [Tier::Planned, Tier::Agentic] {
        let spec = ProposerSpec::new(tier, Some(model("haiku")?), 4)?;
        let result = with_input(|input| proposer.propose(&spec, input))?;
        assert_eq!(result, Err(ProposeError::TierNotInThisRelease { tier }));
    }
    let requests = proposer
        .provider()
        .ok_or("the proposer must hold its provider")?
        .requests()?;
    assert!(requests.is_empty());
    Ok(())
}
