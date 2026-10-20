//! The JSON Schema passed to the model with `--json-schema` (06-CONTEXT D-10, D-12).
//!
//! The model returns `{"proposals": [{"rationale": ..., "files": [{"path": ..., "content": ...}]}]}`: each
//! proposal is the full content of every writable file it changes, never a patch, and `minos-propose` derives the
//! canonical diff itself. The schema narrows what the CLI's own validator accepts (the batch size, the writable
//! paths, the rationale's length, no extra keys), but it is only a first filter: the output is hostile model
//! output either way, and [`crate::proposal::parse_output`] re-checks every rule.

use std::collections::BTreeSet;

use minos_core::limits::{BATCH_MAX, BATCH_MIN};
use minos_core::path::RelPath;
use serde_json::{Map, Value};

use crate::limits::MAX_RATIONALE_CHARS;

/// The output schema text for one proposer call.
///
/// The text is compact JSON with every object's keys in sorted order (they are also inserted in sorted order, so
/// the text would not change if `serde_json`'s map preserved insertion order), so the same batch and writable set
/// always give the same bytes. The argv golden and the golden snapshot depend on that.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSchema {
    text: String,
    batch: u32,
}

/// Why no schema could be built.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The batch lies outside core's `BATCH_MIN..=BATCH_MAX`.
    #[error("batch {batch} is outside {BATCH_MIN} to {BATCH_MAX}")]
    BatchOutOfRange {
        /// The batch that was asked for.
        batch: u32,
    },
    /// The writable set is empty, so no proposal could name a file.
    #[error("the writable set is empty")]
    NoWritable,
}

impl OutputSchema {
    /// The schema for a call that may return up to `batch` proposals over the files in `writable`.
    ///
    /// `proposals` has `minItems` 1 and `maxItems` `batch` (D-12); each proposal has a `rationale` of 1 to
    /// [`MAX_RATIONALE_CHARS`] characters and `files`, 1 to `writable.len()` entries whose `path` is one of the
    /// writable paths in set order; every object has `additionalProperties` false.
    ///
    /// # Errors
    ///
    /// [`SchemaError::BatchOutOfRange`] when `batch` lies outside `BATCH_MIN..=BATCH_MAX`, and
    /// [`SchemaError::NoWritable`] when `writable` is empty.
    pub fn for_batch(batch: u32, writable: &BTreeSet<RelPath>) -> Result<Self, SchemaError> {
        if !(BATCH_MIN..=BATCH_MAX).contains(&batch) {
            return Err(SchemaError::BatchOutOfRange { batch });
        }
        if writable.is_empty() {
            return Err(SchemaError::NoWritable);
        }

        let paths: Vec<Value> = writable
            .iter()
            .map(|path| Value::from(path.as_str()))
            .collect();
        let file = object([
            ("additionalProperties", Value::Bool(false)),
            (
                "properties",
                object([
                    ("content", object([("type", Value::from("string"))])),
                    (
                        "path",
                        object([
                            ("enum", Value::Array(paths)),
                            ("type", Value::from("string")),
                        ]),
                    ),
                ]),
            ),
            ("required", strings(&["path", "content"])),
            ("type", Value::from("object")),
        ]);
        let files = object([
            ("items", file),
            ("maxItems", Value::from(writable.len())),
            ("minItems", Value::from(1)),
            ("type", Value::from("array")),
        ]);
        let rationale = object([
            ("maxLength", Value::from(MAX_RATIONALE_CHARS)),
            ("minLength", Value::from(1)),
            ("type", Value::from("string")),
        ]);
        let proposal = object([
            ("additionalProperties", Value::Bool(false)),
            (
                "properties",
                object([("files", files), ("rationale", rationale)]),
            ),
            ("required", strings(&["rationale", "files"])),
            ("type", Value::from("object")),
        ]);
        let proposals = object([
            ("items", proposal),
            ("maxItems", Value::from(batch)),
            ("minItems", Value::from(1)),
            ("type", Value::from("array")),
        ]);
        let top = object([
            ("additionalProperties", Value::Bool(false)),
            ("properties", object([("proposals", proposals)])),
            ("required", strings(&["proposals"])),
            ("type", Value::from("object")),
        ]);

        // `Value`'s `Display` writes compact JSON and cannot fail.
        Ok(Self {
            text: top.to_string(),
            batch,
        })
    }

    /// The schema as compact JSON text, as passed after `--json-schema`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The largest number of proposals the schema allows.
    #[must_use]
    pub const fn batch(&self) -> u32 {
        self.batch
    }
}

/// A JSON object from `entries`, inserted in the given order.
fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// A JSON array of the strings in `items`, in order.
fn strings(items: &[&str]) -> Value {
    Value::Array(items.iter().map(|item| Value::from(*item)).collect())
}
