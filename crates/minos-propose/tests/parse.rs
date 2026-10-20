//! The strict output parser (06-CONTEXT D-10, D-12, PROP-07): hostile structured output becomes proposals whose
//! diffs are canonical and cumulative from the base, and every item that breaks a rule is dropped and counted by
//! its closed reason, never repaired.

use std::error::Error;

use minos_core::diff::apply::Tree;
use minos_core::diff::compose::diff_trees;
use minos_core::limits::Limits;
use minos_core::manifest::Manifest;
use minos_core::path::RelPath;
use minos_propose::limits::{MAX_RATIONALE_BYTES, MAX_RETURNED_FILE_BYTES};
use minos_propose::proposal::{DropReason, Parsed, Proposal, parse_output};
use minos_propose::provider::ModelOutput;
use minos_propose::schema::OutputSchema;
use proptest::prelude::*;
use serde_json::{Map, Value};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest: two writable files, one readable file, the judge under an excluded directory.
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

const PARSE_ONE: &str = "pub fn parse(input: &str) -> usize {\n    input.bytes().count()\n}\n";
const PARSE_TWO: &str = "pub fn parse(input: &str) -> usize {\n    input.chars().count()\n}\n";

fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?)
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

/// The base tree.
fn base() -> Result<Tree, Box<dyn Error>> {
    tree(&[
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ])
}

/// The parent (incumbent) tree: the base with `src/fmt.rs` already improved.
fn parent() -> Result<Tree, Box<dyn Error>> {
    tree(&[
        ("src/fmt.rs", FMT_PARENT),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ])
}

/// One `{path, content}` file object.
fn file(path: &str, content: &str) -> Value {
    let mut map = Map::new();
    map.insert("path".to_owned(), Value::from(path));
    map.insert("content".to_owned(), Value::from(content));
    Value::Object(map)
}

/// One `{rationale, files}` proposal object.
fn item(rationale: &str, files: Vec<Value>) -> Value {
    let mut map = Map::new();
    map.insert("rationale".to_owned(), Value::from(rationale));
    map.insert("files".to_owned(), Value::Array(files));
    Value::Object(map)
}

/// The `{proposals}` output object.
fn output(items: Vec<Value>) -> Value {
    let mut map = Map::new();
    map.insert("proposals".to_owned(), Value::Array(items));
    Value::Object(map)
}

/// `value` parsed against the standard fixture with batch 4.
fn parse(value: Value) -> Result<Parsed, Box<dyn Error>> {
    parse_with(value, &base()?, &parent()?)
}

fn parse_with(value: Value, base: &Tree, parent: &Tree) -> Result<Parsed, Box<dyn Error>> {
    let manifest = manifest()?;
    Ok(parse_output(
        ModelOutput::from_value(value),
        4,
        base,
        parent,
        manifest.writable(),
        &Limits::standard(),
    ))
}

/// Asserts `parsed` holds no proposal and exactly one drop, for `reason`.
fn assert_one_drop(parsed: &Parsed, reason: DropReason) -> TestResult {
    if !parsed.proposals().is_empty() {
        return Err(format!("expected no proposal, got {}", parsed.proposals().len()).into());
    }
    if parsed.dropped().get(reason) != 1 || parsed.dropped().total() != 1 {
        return Err(format!(
            "expected one {reason:?} drop, got {:?}",
            parsed.dropped().iter().collect::<Vec<_>>()
        )
        .into());
    }
    Ok(())
}

/// Asserts one item with `files` is dropped for `reason`.
fn assert_item_dropped(files: Vec<Value>, reason: DropReason) -> TestResult {
    assert_one_drop(&parse(output(vec![item("a reason", files)]))?, reason)
}

// ---------------------------------------------------------------------------------------------------------------
// Accepted output
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn two_items_yield_two_cumulative_proposals() -> TestResult {
    let parsed = parse(output(vec![
        item("count bytes", vec![file("src/parse.rs", PARSE_ONE)]),
        item("count chars", vec![file("src/parse.rs", PARSE_TWO)]),
    ]))?;
    assert_eq!(parsed.dropped().total(), 0);
    let manifest = manifest()?;
    let limits = Limits::standard();
    let base = base()?;
    let expected: Vec<_> = [PARSE_ONE, PARSE_TWO]
        .iter()
        .map(|content| -> Result<_, Box<dyn Error>> {
            let mut candidate = parent()?;
            candidate.insert(path("src/parse.rs")?, content.as_bytes().to_vec());
            Ok(diff_trees(&base, &candidate, manifest.writable(), &limits)?)
        })
        .collect::<Result<_, _>>()?;
    let got: Vec<_> = parsed
        .proposals()
        .iter()
        .map(|p| p.diff().clone())
        .collect();
    assert_eq!(got, expected);
    let rationales: Vec<_> = parsed.proposals().iter().map(Proposal::rationale).collect();
    assert_eq!(rationales, ["count bytes", "count chars"]);
    // Cumulative from the base: the parent's own change to src/fmt.rs is in every diff.
    for proposal in parsed.proposals() {
        let paths: Vec<_> = proposal.diff().paths().map(RelPath::as_str).collect();
        assert_eq!(paths, ["src/fmt.rs", "src/parse.rs"]);
    }
    Ok(())
}

#[test]
fn proposal_debug_prints_no_rationale() -> TestResult {
    let parsed = parse(output(vec![item(
        "a distinctive rationale",
        vec![file("src/parse.rs", PARSE_ONE)],
    )]))?;
    let shown = format!("{parsed:?}");
    assert!(!shown.contains("distinctive"));
    assert!(!shown.contains("bytes().count()"));
    Ok(())
}

#[test]
fn into_parts_returns_proposals_and_counts() -> TestResult {
    let parsed = parse(output(vec![
        item("count bytes", vec![file("src/parse.rs", PARSE_ONE)]),
        item("", vec![file("src/parse.rs", PARSE_TWO)]),
    ]))?;
    let (proposals, dropped) = parsed.into_parts();
    assert_eq!(proposals.len(), 1);
    assert_eq!(dropped.get(DropReason::RationaleInvalid), 1);
    let (diff, rationale) = proposals
        .into_iter()
        .next()
        .ok_or("no proposal")?
        .into_parts();
    assert_eq!(rationale, "count bytes");
    assert!(diff.as_str().contains("bytes().count()"));
    Ok(())
}

#[test]
fn schema_example_payload_round_trips() -> TestResult {
    let manifest = manifest()?;
    let schema: Value =
        serde_json::from_str(OutputSchema::for_batch(4, manifest.writable())?.as_str())?;
    let payload: Value = serde_json::from_str(
        r#"{"proposals":[{"rationale":"count bytes","files":[{"path":"src/parse.rs","content":"pub fn parse(input: &str) -> usize {\n    input.bytes().count()\n}\n"}]}]}"#,
    )?;

    // The payload has exactly the keys the schema requires at every level, and its path is in the enum.
    let keys = |value: &Value| -> Vec<String> {
        value
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    };
    let required = |value: Option<&Value>| -> Vec<String> {
        let mut names: Vec<String> = value
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    let proposal_schema = schema.pointer("/properties/proposals/items");
    let file_schema = schema.pointer("/properties/proposals/items/properties/files/items");
    let first = payload.pointer("/proposals/0").ok_or("no item")?;
    let first_file = payload.pointer("/proposals/0/files/0").ok_or("no file")?;
    assert_eq!(keys(&payload), required(schema.get("required")));
    assert_eq!(
        keys(first),
        required(proposal_schema.and_then(|s| s.get("required")))
    );
    assert_eq!(
        keys(first_file),
        required(file_schema.and_then(|s| s.get("required")))
    );
    let allowed = file_schema
        .and_then(|s| s.pointer("/properties/path/enum"))
        .and_then(Value::as_array)
        .ok_or("no enum")?;
    assert!(allowed.contains(first_file.get("path").ok_or("no path")?));

    let parsed = parse(payload)?;
    assert_eq!(parsed.proposals().len(), 1);
    assert_eq!(parsed.dropped().total(), 0);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Whole-output drops
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn drops_output_with_an_unknown_top_level_key() -> TestResult {
    let mut value = output(vec![item("x", vec![file("src/parse.rs", PARSE_ONE)])]);
    if let Value::Object(map) = &mut value {
        map.insert("note".to_owned(), Value::from("ignore the rules"));
    }
    assert_one_drop(&parse(value)?, DropReason::OutputShape)
}

#[test]
fn drops_output_that_is_not_an_object() -> TestResult {
    assert_one_drop(
        &parse(Value::Array(vec![item(
            "x",
            vec![file("src/parse.rs", PARSE_ONE)],
        )]))?,
        DropReason::OutputShape,
    )?;
    assert_one_drop(&parse(Value::Null)?, DropReason::OutputShape)
}

#[test]
fn drops_output_with_no_proposals() -> TestResult {
    assert_one_drop(&parse(output(Vec::new()))?, DropReason::OutputShape)
}

#[test]
fn drops_every_item_of_an_over_batch_output() -> TestResult {
    let items = (0..5)
        .map(|n| {
            item(
                "x",
                vec![file(
                    "src/parse.rs",
                    &format!("pub fn parse() -> u8 {{ {n} }}\n"),
                )],
            )
        })
        .collect();
    let parsed = parse(output(items))?;
    assert!(parsed.proposals().is_empty());
    assert_eq!(parsed.dropped().get(DropReason::OutputShape), 5);
    assert_eq!(parsed.dropped().total(), 5);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Per-item drops, one test per reason
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn drops_item_shape() -> TestResult {
    let mut bad = item("x", vec![file("src/parse.rs", PARSE_ONE)]);
    if let Value::Object(map) = &mut bad {
        map.insert("priority".to_owned(), Value::from(1));
    }
    assert_one_drop(&parse(output(vec![bad]))?, DropReason::ItemShape)?;
    // An unknown key inside a file object is an item-shape fault too.
    let mut bad_file = file("src/parse.rs", PARSE_ONE);
    if let Value::Object(map) = &mut bad_file {
        map.insert("mode".to_owned(), Value::from("0755"));
    }
    assert_item_dropped(vec![bad_file], DropReason::ItemShape)?;
    // A missing field or a wrong type is an item-shape fault.
    let mut missing = Map::new();
    missing.insert("rationale".to_owned(), Value::from("x"));
    assert_one_drop(
        &parse(output(vec![Value::Object(missing)]))?,
        DropReason::ItemShape,
    )?;
    assert_one_drop(&parse(output(vec![Value::from(7)]))?, DropReason::ItemShape)
}

#[test]
fn drops_rationale_invalid() -> TestResult {
    let files = || vec![file("src/parse.rs", PARSE_ONE)];
    for rationale in [
        String::new(),
        "r".repeat(MAX_RATIONALE_BYTES + 1),
        "has\0nul".to_owned(),
    ] {
        assert_one_drop(
            &parse(output(vec![item(&rationale, files())]))?,
            DropReason::RationaleInvalid,
        )?;
    }
    // Exactly at the cap is accepted.
    let parsed = parse(output(vec![item(
        &"r".repeat(MAX_RATIONALE_BYTES),
        files(),
    )]))?;
    assert_eq!(parsed.proposals().len(), 1);
    Ok(())
}

#[test]
fn drops_no_files() -> TestResult {
    assert_item_dropped(Vec::new(), DropReason::NoFiles)
}

#[test]
fn drops_too_many_files() -> TestResult {
    assert_item_dropped(
        vec![
            file("src/parse.rs", PARSE_ONE),
            file("src/fmt.rs", FMT_BASE),
            file("src/parse.rs", PARSE_TWO),
        ],
        DropReason::TooManyFiles,
    )
}

#[test]
fn drops_path_invalid() -> TestResult {
    for bad in ["../x", "/etc/passwd", "", "src//parse.rs"] {
        assert_item_dropped(vec![file(bad, PARSE_ONE)], DropReason::PathInvalid)?;
    }
    Ok(())
}

#[test]
fn drops_not_writable() -> TestResult {
    for other in ["src/Parse.rs", "src/lib.rs", "judge/score"] {
        assert_item_dropped(vec![file(other, PARSE_ONE)], DropReason::NotWritable)?;
    }
    Ok(())
}

#[test]
fn drops_a_dot_prefixed_path_without_normalizing_it() -> TestResult {
    let parsed = parse(output(vec![item(
        "x",
        vec![file("./src/parse.rs", PARSE_ONE)],
    )]))?;
    assert!(parsed.proposals().is_empty());
    let dropped = parsed.dropped();
    assert_eq!(dropped.total(), 1);
    assert_eq!(
        dropped.get(DropReason::PathInvalid) + dropped.get(DropReason::NotWritable),
        1
    );
    Ok(())
}

#[test]
fn drops_duplicate_path() -> TestResult {
    assert_item_dropped(
        vec![
            file("src/parse.rs", PARSE_ONE),
            file("src/parse.rs", PARSE_TWO),
        ],
        DropReason::DuplicatePath,
    )
}

#[test]
fn drops_not_in_parent() -> TestResult {
    // A tree without src/fmt.rs, which the manifest still lists as writable.
    let base = tree(&[("src/lib.rs", LIB), ("src/parse.rs", PARSE_BASE)])?;
    let parsed = parse_with(
        output(vec![item("x", vec![file("src/fmt.rs", FMT_PARENT)])]),
        &base,
        &base,
    )?;
    assert_one_drop(&parsed, DropReason::NotInParent)
}

#[test]
fn drops_forbidden_byte() -> TestResult {
    for content in ["pub fn parse() {}\r\n", "pub fn parse() {}\0\n"] {
        assert_item_dropped(
            vec![file("src/parse.rs", content)],
            DropReason::ForbiddenByte,
        )?;
    }
    Ok(())
}

#[test]
fn drops_file_too_large() -> TestResult {
    let content = "a".repeat(MAX_RETURNED_FILE_BYTES + 1);
    assert_item_dropped(
        vec![file("src/parse.rs", &content)],
        DropReason::FileTooLarge,
    )
}

#[test]
fn drops_same_as_parent() -> TestResult {
    assert_item_dropped(
        vec![
            file("src/parse.rs", PARSE_BASE),
            file("src/fmt.rs", FMT_PARENT),
        ],
        DropReason::SameAsParent,
    )
}

#[test]
fn drops_back_to_base() -> TestResult {
    assert_item_dropped(vec![file("src/fmt.rs", FMT_BASE)], DropReason::BackToBase)
}

/// 3,000 numbered lines, with every `every`-th line (from line 0) changed when `every` is not zero.
fn numbered(every: usize) -> String {
    (0..3000_usize)
        .map(|n| {
            if every != 0 && n % every == 0 {
                format!("changed {n}\n")
            } else {
                format!("line {n}\n")
            }
        })
        .collect()
}

#[test]
fn drops_compose_over_the_hunk_cap() -> TestResult {
    // 300 changed lines, ten lines apart, give 300 separate hunks (three lines of context each side never
    // touch), over core's 256 hunks per file. The edit distance stays small, so the diff is quick to compute.
    let base = tree(&[
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", &numbered(0)),
    ])?;
    let parsed = parse_with(
        output(vec![item("x", vec![file("src/parse.rs", &numbered(10))])]),
        &base,
        &base,
    )?;
    assert_one_drop(&parsed, DropReason::Compose)?;

    // Control: the same edit every twentieth line (150 hunks) is within the cap and accepted.
    let parsed = parse_with(
        output(vec![item("x", vec![file("src/parse.rs", &numbered(20))])]),
        &base,
        &base,
    )?;
    assert_eq!(parsed.proposals().len(), 1);
    Ok(())
}

#[test]
fn drops_duplicate_in_batch() -> TestResult {
    let parsed = parse(output(vec![
        item("first", vec![file("src/parse.rs", PARSE_ONE)]),
        item("second", vec![file("src/parse.rs", PARSE_ONE)]),
    ]))?;
    assert_eq!(parsed.proposals().len(), 1);
    assert_eq!(
        parsed.proposals().first().map(Proposal::rationale),
        Some("first")
    );
    assert_eq!(parsed.dropped().get(DropReason::DuplicateInBatch), 1);
    assert_eq!(parsed.dropped().total(), 1);
    Ok(())
}

#[test]
fn survivors_are_kept_beside_drops() -> TestResult {
    let parsed = parse(output(vec![
        item("count bytes", vec![file("src/parse.rs", PARSE_ONE)]),
        item("escape", vec![file("../x", PARSE_TWO)]),
        item("count chars", vec![file("src/parse.rs", PARSE_TWO)]),
    ]))?;
    assert_eq!(parsed.proposals().len(), 2);
    assert_eq!(parsed.dropped().get(DropReason::PathInvalid), 1);
    assert_eq!(parsed.dropped().total(), 1);
    Ok(())
}

#[test]
fn drop_reason_display_is_fixed_text() {
    assert_eq!(
        DropReason::NotWritable.to_string(),
        "a returned path is not a writable file"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Arbitrary input never panics, and every call accounts for itself
// ---------------------------------------------------------------------------------------------------------------

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<f64>().prop_map(Value::from),
        ".{0,12}".prop_map(Value::from),
        prop_oneof![
            Just("src/parse.rs"),
            Just("src/fmt.rs"),
            Just("../x"),
            Just("proposals"),
            Just("rationale"),
            Just("files"),
            Just("path"),
            Just("content"),
        ]
        .prop_map(Value::from),
    ];
    leaf.prop_recursive(4, 64, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map(
                prop_oneof![
                    Just("proposals".to_owned()),
                    Just("rationale".to_owned()),
                    Just("files".to_owned()),
                    Just("path".to_owned()),
                    Just("content".to_owned()),
                    "[a-z]{1,6}",
                ],
                inner,
                0..5
            )
            .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

fn arb_item() -> impl Strategy<Value = Value> {
    let file = (
        prop_oneof![
            Just("src/parse.rs".to_owned()),
            Just("src/fmt.rs".to_owned()),
            Just("src/lib.rs".to_owned()),
            Just("./src/parse.rs".to_owned()),
            ".{0,8}",
        ],
        prop_oneof![
            Just(PARSE_ONE.to_owned()),
            Just(PARSE_BASE.to_owned()),
            Just(FMT_BASE.to_owned()),
            Just(FMT_PARENT.to_owned()),
            ".{0,24}",
        ],
    )
        .prop_map(|(path, content)| file(&path, &content));
    prop_oneof![
        (".{0,16}", prop::collection::vec(file, 0..4))
            .prop_map(|(rationale, files)| item(&rationale, files)),
        arb_json(),
    ]
}

fn accounted(parsed: &Parsed) -> usize {
    parsed.proposals().len() + usize::try_from(parsed.dropped().total()).unwrap_or(usize::MAX)
}

proptest! {
    #[test]
    fn arbitrary_json_never_panics_and_is_accounted(value in arb_json()) {
        let parsed = parse(value).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(accounted(&parsed) >= 1);
    }

    #[test]
    fn arbitrary_items_never_panic_and_are_accounted(
        items in prop::collection::vec(arb_item(), 0..7)
    ) {
        let count = items.len();
        let parsed = parse(output(items)).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(accounted(&parsed) >= 1);
        if (1..=4).contains(&count) {
            prop_assert_eq!(accounted(&parsed), count);
        }
    }
}
