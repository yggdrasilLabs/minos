//! The single-shot tier (PROP-07, PROP-08, PROP-09, 06-CONTEXT D-10, D-12): one model call through any
//! `ModelProvider`, strict parsing of the raw output, every drop counted, token usage and origin returned, and
//! typed provider stops and failures. Every case runs through the generic [`propose_with`], so the same tier code
//! serves `FakeProvider` here and `ClaudeCli` later.

use std::collections::BTreeMap;
use std::error::Error;

use minos_core::diff::apply::{Tree, apply};
use minos_core::id::{ManifestId, ModelId, PromptHash, RunId, TreeId};
use minos_core::ledger::{Origin, StopReason};
use minos_core::manifest::{Manifest, ProposerSpec, Tier};
use minos_core::path::RelPath;
use minos_core::usage::Usage;
use minos_propose::fake::{FakeProvider, FakeReply, RecordedRequest};
use minos_propose::prompt::History;
use minos_propose::proposal::DropReason;
use minos_propose::proposer::{Proposals, ProposeError, ProposeInput};
use minos_propose::provider::{EmptyCause, InfraCause, ModelProvider, ProviderError, ProviderStop};
use minos_propose::schema::OutputSchema;
use minos_propose::single_shot;
use serde_json::{Map, Value};

type TestResult = Result<(), Box<dyn Error>>;

/// One call's result and the requests the fake recorded.
type OneCall = (Result<Proposals, ProposeError>, Vec<RecordedRequest>);

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs", "src/fmt.rs"]
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

const PARSE_BASE: &str = "pub fn parse(input: &str) -> usize {\n    input.len()\n}\n";
const FMT_BASE: &str = "pub fn fmt(n: usize) -> String {\n    n.to_string()\n}\n";
const FMT_PARENT: &str = "pub fn fmt(n: usize) -> String {\n    format!(\"{n}\")\n}\n";
const LIB: &str = "pub mod fmt;\npub mod parse;\n";
const PROGRAM: &[u8] = b"Make parsing faster.\n";

const PARSE_ONE: &str = "pub fn parse(input: &str) -> usize {\n    input.bytes().count()\n}\n";
const PARSE_TWO: &str = "pub fn parse(input: &str) -> usize {\n    input.chars().count()\n}\n";

fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?)
}

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

fn model(name: &str) -> Result<ModelId, Box<dyn Error>> {
    Ok(name.parse::<ModelId>()?)
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

/// The base with `src/fmt.rs` already improved.
fn parent() -> Result<Tree, Box<dyn Error>> {
    tree(&[
        ("src/fmt.rs", FMT_PARENT),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ])
}

fn spec(batch: u32) -> Result<ProposerSpec, Box<dyn Error>> {
    Ok(ProposerSpec::new(
        Tier::SingleShot,
        Some(model("haiku")?),
        batch,
    )?)
}

fn usage() -> Usage {
    Usage::tokens(10, 20, 30, 40)
}

fn file(path: &str, content: &str) -> Value {
    let mut map = Map::new();
    map.insert("path".to_owned(), Value::from(path));
    map.insert("content".to_owned(), Value::from(content));
    Value::Object(map)
}

fn item(rationale: &str, files: Vec<Value>) -> Value {
    let mut map = Map::new();
    map.insert("rationale".to_owned(), Value::from(rationale));
    map.insert("files".to_owned(), Value::Array(files));
    Value::Object(map)
}

fn output(items: Vec<Value>) -> Value {
    let mut map = Map::new();
    map.insert("proposals".to_owned(), Value::Array(items));
    Value::Object(map)
}

fn structured(items: Vec<Value>, models: BTreeMap<ModelId, Usage>) -> FakeReply {
    FakeReply::Structured {
        output: output(items),
        usage: usage(),
        models,
    }
}

fn one_model(name: &str) -> Result<BTreeMap<ModelId, Usage>, Box<dyn Error>> {
    Ok(BTreeMap::from([(model(name)?, usage())]))
}

/// The single-shot tier over any provider: the one shape every case uses, and the shape `ClaudeCli` is called
/// with later.
fn propose_with<P: ModelProvider>(
    provider: &P,
    spec: &ProposerSpec,
    manifest: &Manifest,
    base: &Tree,
    parent: &Tree,
) -> Result<Proposals, ProposeError> {
    let history = History::empty();
    let run = RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), 1_760_000_000);
    let input = ProposeInput {
        manifest,
        run: &run,
        generation: 1,
        base,
        parent,
        program: PROGRAM,
        history: &history,
        notes: &[],
    };
    single_shot::propose(provider, spec, &input)
}

/// Runs the standard fixture (batch 4) against a fake scripted with `reply`.
fn run_one(reply: FakeReply) -> Result<OneCall, Box<dyn Error>> {
    let fake = FakeProvider::new(vec![reply]);
    let result = propose_with(&fake, &spec(4)?, &manifest()?, &base()?, &parent()?);
    Ok((result, fake.requests()?))
}

// ---------------------------------------------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn two_valid_proposals_carry_usage_and_the_resolved_model() -> TestResult {
    let reply = structured(
        vec![
            item("count bytes", vec![file("src/parse.rs", PARSE_ONE)]),
            item("count chars", vec![file("src/parse.rs", PARSE_TWO)]),
        ],
        one_model("claude-haiku-5-5")?,
    );
    let (result, requests) = run_one(reply)?;
    let proposals = result?;
    let [request] = requests.as_slice() else {
        return Err("expected exactly one request".into());
    };

    assert_eq!(proposals.tier(), Tier::SingleShot);
    assert_eq!(proposals.items().len(), 2);
    assert_eq!(proposals.dropped().total(), 0);
    assert_eq!(proposals.usage(), Some(&usage()));
    assert_eq!(proposals.models(), &one_model("claude-haiku-5-5")?);
    assert_eq!(
        proposals.origin(),
        &Origin::Model {
            model: model("claude-haiku-5-5")?,
            prompt: PromptHash::of(request.prompt()),
        }
    );
    assert_eq!(request.model(), &model("haiku")?);
    let manifest = manifest()?;
    let schema = OutputSchema::for_batch(4, manifest.writable())?;
    assert_eq!(request.schema(), schema.as_str());

    // Each diff is cumulative from the base: it carries the parent's edit too.
    let base = base()?;
    for proposal in proposals.items() {
        let applied = apply(&base, proposal.diff())?;
        assert_eq!(
            applied.get(&path("src/fmt.rs")?),
            Some(FMT_PARENT.as_bytes())
        );
    }
    Ok(())
}

#[test]
fn the_requested_model_is_the_origin_unless_exactly_one_resolved() -> TestResult {
    let two = BTreeMap::from([
        (model("claude-haiku-5-5")?, usage()),
        (model("claude-sonnet-5-5")?, usage()),
    ]);
    for models in [BTreeMap::new(), two] {
        let reply = structured(
            vec![item("count bytes", vec![file("src/parse.rs", PARSE_ONE)])],
            models,
        );
        let (result, requests) = run_one(reply)?;
        let proposals = result?;
        let [request] = requests.as_slice() else {
            return Err("expected exactly one request".into());
        };
        assert_eq!(
            proposals.origin(),
            &Origin::Model {
                model: model("haiku")?,
                prompt: PromptHash::of(request.prompt()),
            }
        );
    }
    Ok(())
}

#[test]
fn invalid_items_are_dropped_and_counted() -> TestResult {
    let reply = structured(
        vec![
            item("count bytes", vec![file("src/parse.rs", PARSE_ONE)]),
            item("edit lib", vec![file("src/lib.rs", "pub mod parse;\n")]),
            item("crlf", vec![file("src/parse.rs", "fn a() {}\r\n")]),
        ],
        BTreeMap::new(),
    );
    let (result, _) = run_one(reply)?;
    let proposals = result?;
    assert_eq!(proposals.items().len(), 1);
    assert_eq!(proposals.dropped().get(DropReason::NotWritable), 1);
    assert_eq!(proposals.dropped().get(DropReason::ForbiddenByte), 1);
    assert_eq!(proposals.dropped().total(), 2);
    Ok(())
}

#[test]
fn zero_survivors_is_an_empty_call_not_an_error() -> TestResult {
    let reply = structured(
        vec![
            item("", vec![file("src/parse.rs", PARSE_ONE)]),
            item("no change", vec![file("src/parse.rs", PARSE_BASE)]),
            item("edit lib", vec![file("src/lib.rs", "pub mod parse;\n")]),
        ],
        BTreeMap::new(),
    );
    let (result, _) = run_one(reply)?;
    let proposals = result?;
    assert!(proposals.items().is_empty());
    assert_eq!(proposals.dropped().total(), 3);
    assert!(proposals.empty().is_none());
    assert_eq!(proposals.usage(), Some(&usage()));
    Ok(())
}

#[test]
fn an_empty_completion_is_ok_with_its_cause_and_usage() -> TestResult {
    let (result, requests) = run_one(FakeReply::Empty {
        cause: EmptyCause::MaxTurns,
        usage: usage(),
    })?;
    let proposals = result?;
    assert!(proposals.items().is_empty());
    assert_eq!(proposals.empty(), Some(EmptyCause::MaxTurns));
    assert_eq!(proposals.usage(), Some(&usage()));
    assert_eq!(requests.len(), 1);
    Ok(())
}

#[test]
fn provider_stops_and_failures_are_typed() -> TestResult {
    let (limit, _) = run_one(FakeReply::Error(ProviderError::Stop {
        stop: ProviderStop::Limit,
        usage: Some(usage()),
    }))?;
    let Err(limit) = limit else {
        return Err("a limit must be an error".into());
    };
    assert!(matches!(limit, ProposeError::Provider(_)));
    assert_eq!(limit.stop_reason(), Some(StopReason::ProviderLimit));
    assert_eq!(limit.usage(), Some(&usage()));
    assert!(!limit.is_infra());

    let (auth, _) = run_one(FakeReply::Error(ProviderError::Stop {
        stop: ProviderStop::Auth,
        usage: None,
    }))?;
    let Err(auth) = auth else {
        return Err("an auth failure must be an error".into());
    };
    assert_eq!(auth.stop_reason(), Some(StopReason::ProviderAuth));

    let (infra, _) = run_one(FakeReply::Error(ProviderError::Infra {
        cause: InfraCause::Deadline,
        usage: None,
    }))?;
    let Err(infra) = infra else {
        return Err("an infrastructure failure must be an error".into());
    };
    assert!(infra.is_infra());
    assert_eq!(infra.stop_reason(), None);
    Ok(())
}

#[test]
fn configuration_errors_make_no_request() -> TestResult {
    let manifest = manifest()?;
    let base = base()?;
    let parent = parent()?;
    let reply = || structured(vec![], BTreeMap::new());

    let fake = FakeProvider::new(vec![reply()]);
    let sweep = ProposerSpec::new(Tier::Sweep, None, 4)?;
    assert_eq!(
        propose_with(&fake, &sweep, &manifest, &base, &parent),
        Err(ProposeError::TierMismatch {
            expected: Tier::SingleShot,
            found: Tier::Sweep,
        })
    );
    assert!(fake.requests()?.is_empty());

    let fake = FakeProvider::new(vec![reply()]);
    let no_model = ProposerSpec::new(Tier::SingleShot, None, 4)?;
    assert_eq!(
        propose_with(&fake, &no_model, &manifest, &base, &parent),
        Err(ProposeError::NoModel)
    );
    assert!(fake.requests()?.is_empty());

    // Base and parent agree on their paths, but the writable `src/parse.rs` is in neither.
    let fake = FakeProvider::new(vec![reply()]);
    let short = tree(&[("src/fmt.rs", FMT_BASE), ("src/lib.rs", LIB)])?;
    let result = propose_with(&fake, &spec(4)?, &manifest, &short, &short);
    assert!(matches!(result, Err(ProposeError::Prompt(_))));
    assert!(fake.requests()?.is_empty());
    Ok(())
}

#[test]
fn an_exhausted_script_is_an_infra_failure_and_requests_keep_call_order() -> TestResult {
    let manifest = manifest()?;
    let base = base()?;
    let parent = parent()?;
    let fake = FakeProvider::new(vec![
        FakeReply::Empty {
            cause: EmptyCause::Refusal,
            usage: usage(),
        },
        FakeReply::Empty {
            cause: EmptyCause::MaxTurns,
            usage: usage(),
        },
    ]);
    propose_with(&fake, &spec(4)?, &manifest, &base, &parent)?;
    propose_with(&fake, &spec(2)?, &manifest, &base, &parent)?;
    let exhausted = propose_with(&fake, &spec(3)?, &manifest, &base, &parent);
    assert_eq!(
        exhausted,
        Err(ProposeError::Provider(ProviderError::Infra {
            cause: InfraCause::Fake,
            usage: None,
        }))
    );

    let schemas: Vec<String> = fake
        .requests()?
        .iter()
        .map(|request| request.schema().to_owned())
        .collect();
    let expected = [4, 2, 3]
        .into_iter()
        .map(|batch| {
            OutputSchema::for_batch(batch, manifest.writable()).map(|s| s.as_str().to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(schemas, expected);
    Ok(())
}
