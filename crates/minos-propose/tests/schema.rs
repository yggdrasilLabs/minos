//! The output schema passed to the model with `--json-schema` (06-CONTEXT D-10, D-12): a golden snapshot, a byte
//! pin, its structure, determinism, and refusals.
//!
//! The schema text is an insta snapshot under `tests/snapshots/`. `.config/insta.yaml` sets `update: no`, so the
//! plain gate never writes or accepts one: the snapshot is created deliberately with
//! `INSTA_UPDATE=always cargo test -p minos-propose --test schema` and reviewed before it is committed. insta trims
//! trailing whitespace and normalizes line endings when it compares, so `output_schema_bytes_pinned` also pins
//! the exact bytes with one SHA-256 literal (03-RESEARCH Pitfall 4).

use std::collections::BTreeSet;
use std::error::Error;

use minos_core::limits::{BATCH_MAX, BATCH_MIN};
use minos_core::path::RelPath;
use minos_propose::limits::MAX_RATIONALE_BYTES;
use minos_propose::schema::{OutputSchema, SchemaError};
use serde_json::Value;
use sha2::{Digest, Sha256};

type TestResult = Result<(), Box<dyn Error>>;

fn writable(paths: &[&str]) -> Result<BTreeSet<RelPath>, Box<dyn Error>> {
    let mut set = BTreeSet::new();
    for path in paths {
        set.insert(RelPath::try_from(*path)?);
    }
    Ok(set)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

/// The value at `path` (object keys, or array indices written as numbers) inside `value`.
fn at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, Box<dyn Error>> {
    let mut here = value;
    for key in path {
        here = here.get(*key).ok_or_else(|| format!("missing key {key}"))?;
    }
    Ok(here)
}

fn parsed(schema: &OutputSchema) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_str(schema.as_str())?)
}

#[test]
fn output_schema_golden() -> TestResult {
    let schema = OutputSchema::for_batch(4, &writable(&["src/parse.rs"])?)?;
    insta::assert_snapshot!("output_schema_batch_4", schema.as_str());
    Ok(())
}

#[test]
fn output_schema_bytes_pinned() -> TestResult {
    // The pin was computed outside the code under test, from the accepted snapshot body (the text after the
    // file's second `---` line), which `awk` prints followed by one newline:
    //
    //   cd crates/minos-propose/tests/snapshots
    //   awk 'n >= 2 { print } /^---$/ { n++ }' schema__output_schema_batch_4.snap | shasum -a 256
    //
    // so the bytes hashed here are the schema text plus one newline.
    let schema = OutputSchema::for_batch(4, &writable(&["src/parse.rs"])?)?;
    let mut text = schema.as_str().to_owned();
    text.push('\n');
    assert_eq!(
        sha256_hex(text.as_bytes()),
        "bd6ccb83d04d7df1d09f29c528e0bdc707e96087445140ff8bd22c2f1e41fd83"
    );
    Ok(())
}

#[test]
fn output_schema_structure() -> TestResult {
    let schema = OutputSchema::for_batch(4, &writable(&["src/parse.rs"])?)?;
    assert_eq!(schema.batch(), 4);
    let value = parsed(&schema)?;
    let proposals = at(&value, &["properties", "proposals"])?;
    let item = at(proposals, &["items"])?;
    let files = at(item, &["properties", "files"])?;
    let file = at(files, &["items"])?;

    assert_eq!(at(proposals, &["maxItems"])?, &Value::from(4));
    assert_eq!(at(proposals, &["minItems"])?, &Value::from(1));
    assert_eq!(at(files, &["maxItems"])?, &Value::from(1));
    assert_eq!(at(files, &["minItems"])?, &Value::from(1));
    assert_eq!(
        at(file, &["properties", "path", "enum"])?,
        &Value::Array(vec![Value::from("src/parse.rs")])
    );
    assert_eq!(
        at(item, &["properties", "rationale", "maxLength"])?,
        &Value::from(MAX_RATIONALE_BYTES)
    );
    assert_eq!(
        at(item, &["properties", "rationale", "minLength"])?,
        &Value::from(1)
    );
    for object in [&value, item, file] {
        assert_eq!(at(object, &["additionalProperties"])?, &Value::Bool(false));
        assert_eq!(at(object, &["type"])?, &Value::from("object"));
    }
    assert_eq!(
        at(&value, &["required"])?,
        &Value::Array(vec![Value::from("proposals")])
    );
    assert_eq!(
        at(item, &["required"])?,
        &Value::Array(vec![Value::from("rationale"), Value::from("files")])
    );
    assert_eq!(
        at(file, &["required"])?,
        &Value::Array(vec![Value::from("path"), Value::from("content")])
    );
    Ok(())
}

#[test]
fn output_schema_lists_writable_paths_in_set_order() -> TestResult {
    let schema = OutputSchema::for_batch(2, &writable(&["src/z.rs", "src/a.rs", "lib/m.rs"])?)?;
    let value = parsed(&schema)?;
    let file = at(
        &value,
        &["properties", "proposals", "items", "properties", "files"],
    )?;
    assert_eq!(at(file, &["maxItems"])?, &Value::from(3));
    assert_eq!(
        at(file, &["items", "properties", "path", "enum"])?,
        &Value::Array(vec![
            Value::from("lib/m.rs"),
            Value::from("src/a.rs"),
            Value::from("src/z.rs")
        ])
    );
    Ok(())
}

#[test]
fn output_schema_is_deterministic() -> TestResult {
    let set = writable(&["src/parse.rs", "src/fmt.rs"])?;
    let first = OutputSchema::for_batch(4, &set)?;
    let second = OutputSchema::for_batch(4, &set)?;
    assert_eq!(first.as_str(), second.as_str());
    assert!(!first.as_str().contains('\n'));
    Ok(())
}

#[test]
fn output_schema_accepts_the_batch_bounds() -> TestResult {
    let set = writable(&["src/parse.rs"])?;
    for batch in [BATCH_MIN, BATCH_MAX] {
        let schema = OutputSchema::for_batch(batch, &set)?;
        let value = parsed(&schema)?;
        assert_eq!(
            at(&value, &["properties", "proposals", "maxItems"])?,
            &Value::from(batch)
        );
    }
    Ok(())
}

#[test]
fn output_schema_refuses_batch_out_of_range() -> TestResult {
    let set = writable(&["src/parse.rs"])?;
    for batch in [0, BATCH_MAX + 1] {
        match OutputSchema::for_batch(batch, &set) {
            Err(SchemaError::BatchOutOfRange { batch: got }) => assert_eq!(got, batch),
            Err(other) => return Err(format!("batch {batch}: wrong error {other}").into()),
            Ok(_) => return Err(format!("batch {batch} accepted").into()),
        }
    }
    Ok(())
}

#[test]
fn output_schema_refuses_no_writable() {
    assert!(matches!(
        OutputSchema::for_batch(4, &BTreeSet::new()),
        Err(SchemaError::NoWritable)
    ));
}
