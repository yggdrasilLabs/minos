//! The strict exact-position applier, cumulative composition, and `Experiment` (CORE-05, 03-CONTEXT D-02 to D-04).
//!
//! Diffs are built from `diffy::create_patch` output with canonical headers, or written inline, and always go
//! through `Diff::parse`, so every diff the applier sees here is one a candidate could have produced.

use std::collections::BTreeSet;
use std::error::Error;

use minos_core::diff::DiffError;
use minos_core::diff::apply::{ApplyError, Tree, apply};
use minos_core::diff::compose::{ComposeError, compose, diff_trees};
use minos_core::diff::cumulative::{BaseTree, CumulativeDiff};
use minos_core::diff::{Diff, FileDiff, HunkLine};
use minos_core::experiment::Experiment;
use minos_core::id::{ExperimentId, TreeId};
use minos_core::limits::Limits;
use minos_core::outcome::Reason;
use minos_core::path::{PathError, RelPath};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// The one writable path most tests use.
const SRC: &str = "src/parse.rs";

/// A second writable path, never changed by the single-file tests.
const LIB: &str = "src/lib.rs";

fn rel(path: &str) -> Result<RelPath, PathError> {
    RelPath::try_from(path)
}

fn writable() -> Result<BTreeSet<RelPath>, PathError> {
    [SRC, LIB].into_iter().map(rel).collect()
}

/// Canonical headers for `path`, then diffy's hunks for `base` to `modified` (diffy's own two header lines are
/// dropped). `None` if diffy's output does not have the expected shape.
fn create_patch_text(path: &str, base: &str, modified: &str) -> Option<String> {
    let patch = diffy::create_patch(base, modified).to_string();
    let (_, rest) = patch.split_once('\n')?;
    let (_, hunks) = rest.split_once('\n')?;
    Some(format!(
        "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n{hunks}"
    ))
}

fn parse(text: &str) -> Result<Diff, Box<dyn Error>> {
    Ok(Diff::parse(text, &writable()?, &Limits::standard())?)
}

/// The parsed diff from `base` to `modified` on [`SRC`].
fn diff_of(base: &str, modified: &str) -> Result<Diff, Box<dyn Error>> {
    let text = create_patch_text(SRC, base, modified).ok_or("diffy output shape")?;
    parse(&text)
}

/// A tree holding [`SRC`] with `src` and [`LIB`] with fixed content.
fn tree_with(src: &[u8]) -> Result<Tree, Box<dyn Error>> {
    let mut tree = Tree::new();
    tree.insert(rel(SRC)?, src.to_vec());
    tree.insert(rel(LIB)?, b"pub mod parse;\n".to_vec());
    Ok(tree)
}

fn src_of(tree: &Tree) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(tree.get(&rel(SRC)?).ok_or("src missing")?.to_vec())
}

fn applied(base: &str, diff: &Diff) -> Result<Vec<u8>, Box<dyn Error>> {
    src_of(&apply(&tree_with(base.as_bytes())?, diff)?)
}

/// `line 1\n` to `line N\n`.
fn numbered(count: u32) -> String {
    (1..=count).fold(String::new(), |mut text, n| {
        text.push_str("line ");
        text.push_str(&n.to_string());
        text.push('\n');
        text
    })
}

fn fail<E: std::fmt::Display>(error: E) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

/// File contents over a small line alphabet, with and without a final newline.
fn file_text() -> impl Strategy<Value = String> {
    text_of(0)
}

/// Like [`file_text`], with at least one line, so a diff against it always reads a base line.
fn non_empty_text() -> impl Strategy<Value = String> {
    text_of(1)
}

fn text_of(min_lines: usize) -> impl Strategy<Value = String> {
    let line = prop::sample::select(vec!["a", "b", "c", "", " d", "-- a/x", "\\ No newline"]);
    (prop::collection::vec(line, min_lines..12), any::<bool>()).prop_map(
        |(lines, final_newline)| {
            let mut text = lines.join("\n");
            if final_newline && !lines.is_empty() {
                text.push('\n');
            }
            text
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Task 1: Tree and apply
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn tree_accessors() -> TestResult {
    let mut tree = Tree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.insert(rel(SRC)?, b"one\n".to_vec()), None);
    assert_eq!(
        tree.insert(rel(SRC)?, b"two\n".to_vec()),
        Some(b"one\n".to_vec())
    );
    tree.insert(rel(LIB)?, b"lib\n".to_vec());
    assert_eq!(tree.len(), 2);
    assert!(!tree.is_empty());
    assert!(tree.contains(&rel(SRC)?));
    assert!(!tree.contains(&rel("src/main.rs")?));
    assert_eq!(tree.get(&rel(SRC)?), Some(&b"two\n"[..]));
    let paths: Vec<&str> = tree.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, vec![LIB, SRC]);
    assert_eq!(Tree::default(), Tree::new());
    Ok(())
}

#[test]
fn applies_a_simple_modify() -> TestResult {
    let base = "fn parse() {\n    old();\n}\n";
    let modified = "fn parse() {\n    new();\n}\n";
    assert_eq!(
        applied(base, &diff_of(base, modified)?)?,
        modified.as_bytes()
    );
    Ok(())
}

#[test]
fn applies_several_hunks_in_one_file() -> TestResult {
    let base = numbered(30);
    let modified = base
        .replace("line 2\n", "line two\n")
        .replace("line 28\n", "line twenty-eight\nextra\n");
    let diff = diff_of(&base, &modified)?;
    assert_eq!(diff.files().first().map(|file| file.hunks().len()), Some(2));
    assert_eq!(applied(&base, &diff)?, modified.as_bytes());
    Ok(())
}

#[test]
fn inserts_after_a_stated_line() -> TestResult {
    let base = "1\n2\n3\n4\n5\n6\n";
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -4,0 +5,1 @@\n+new\n",
    )?;
    assert_eq!(applied(base, &diff)?, b"1\n2\n3\n4\nnew\n5\n6\n");
    Ok(())
}

#[test]
fn inserts_at_the_top() -> TestResult {
    let base = "1\n2\n";
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -0,0 +1,1 @@\n+top\n",
    )?;
    assert_eq!(applied(base, &diff)?, b"top\n1\n2\n");
    Ok(())
}

#[test]
fn inserts_at_the_end() -> TestResult {
    let base = "1\n2\n";
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -2,0 +3,1 @@\n+end\n",
    )?;
    assert_eq!(applied(base, &diff)?, b"1\n2\nend\n");
    Ok(())
}

#[test]
fn removes_and_adds_the_final_newline() -> TestResult {
    for (base, modified) in [
        ("a\nb\n", "a\nb"),
        ("a\nb", "a\nb\n"),
        ("b", "b\n"),
        ("b\n", "b"),
        ("a\nb", "a\nc"),
    ] {
        assert_eq!(
            applied(base, &diff_of(base, modified)?)?,
            modified.as_bytes(),
            "{base:?} -> {modified:?}"
        );
    }
    Ok(())
}

#[test]
fn applies_to_an_empty_file() -> TestResult {
    assert_eq!(applied("", &diff_of("", "first\n")?)?, b"first\n");
    Ok(())
}

#[test]
fn rejects_a_context_mismatch() -> TestResult {
    let diff = diff_of("a\nb\nc\n", "a\nB\nc\n")?;
    let error = apply(&tree_with(b"a\nb\nX\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert_eq!(
        error,
        ApplyError::ContextMismatch {
            path: rel(SRC)?,
            hunk: 1,
            line: 3
        }
    );
    Ok(())
}

#[test]
fn rejects_a_newline_presence_mismatch() -> TestResult {
    // The diff's last context line has a newline; the base's lacks one.
    let diff = diff_of("a\nb\n", "A\nb\n")?;
    let error = apply(&tree_with(b"a\nb")?, &diff).err().ok_or("applied")?;
    assert_eq!(
        error,
        ApplyError::ContextMismatch {
            path: rel(SRC)?,
            hunk: 1,
            line: 2
        }
    );
    Ok(())
}

#[test]
fn never_searches_another_offset() -> TestResult {
    // The same lines exist two lines further down; a fuzzy applier would find them there.
    let diff = diff_of("a\nb\nc\n", "a\nB\nc\n")?;
    let error = apply(&tree_with(b"x\ny\na\nb\nc\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert!(matches!(
        error,
        ApplyError::ContextMismatch {
            hunk: 1,
            line: 1,
            ..
        }
    ));
    Ok(())
}

#[test]
fn rejects_a_missing_file() -> TestResult {
    let diff = diff_of("a\n", "b\n")?;
    let mut tree = Tree::new();
    tree.insert(rel(LIB)?, b"a\n".to_vec());
    let error = apply(&tree, &diff).err().ok_or("applied")?;
    assert_eq!(error, ApplyError::MissingFile { path: rel(SRC)? });
    assert_eq!(
        error.reason(),
        Reason::ApplyFailed {
            path: rel(SRC)?,
            hunk: 0
        }
    );
    Ok(())
}

#[test]
fn rejects_a_hunk_past_the_end() -> TestResult {
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -9,1 +9,1 @@\n-a\n+b\n",
    )?;
    let error = apply(&tree_with(b"a\nb\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert_eq!(
        error,
        ApplyError::OutOfRange {
            path: rel(SRC)?,
            hunk: 1
        }
    );
    assert_eq!(
        error.reason(),
        Reason::ApplyFailed {
            path: rel(SRC)?,
            hunk: 1
        }
    );
    let insert = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -3,0 +4,1 @@\n+b\n",
    )?;
    let error = apply(&tree_with(b"a\nb\n")?, &insert)
        .err()
        .ok_or("applied")?;
    assert!(matches!(error, ApplyError::OutOfRange { hunk: 1, .. }));
    Ok(())
}

#[test]
fn rejects_a_hunk_that_runs_past_the_end() -> TestResult {
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -2,2 +2,2 @@\n b\n-c\n+C\n",
    )?;
    let error = apply(&tree_with(b"a\nb\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert!(matches!(error, ApplyError::OutOfRange { hunk: 1, .. }));
    Ok(())
}

#[test]
fn rejects_an_insert_after_a_line_without_newline() -> TestResult {
    // Appending after a last line that lacks its newline would change that line; a real diff deletes and re-adds it.
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -1,0 +2,1 @@\n+b\n",
    )?;
    let error = apply(&tree_with(b"a")?, &diff).err().ok_or("applied")?;
    assert!(matches!(error, ApplyError::OutOfRange { hunk: 1, .. }));
    Ok(())
}

#[test]
fn rejects_a_new_end_of_file_before_the_base_ends() -> TestResult {
    // The new side claims the file ends after `x`, but the base has more lines after the hunk.
    let diff = parse(
        "diff --git a/src/parse.rs b/src/parse.rs\n--- a/src/parse.rs\n+++ b/src/parse.rs\n@@ -1,1 +1,1 @@\n-a\n+x\n\\ No newline at end of file\n",
    )?;
    let error = apply(&tree_with(b"a\nb\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert!(matches!(error, ApplyError::OutOfRange { hunk: 1, .. }));
    Ok(())
}

#[test]
fn reports_the_failing_hunk_number() -> TestResult {
    let base = numbered(30);
    let modified = base
        .replace("line 2\n", "line two\n")
        .replace("line 28\n", "line twenty-eight\n");
    let diff = diff_of(&base, &modified)?;
    let broken = base.replace("line 27\n", "changed\n");
    let error = apply(&tree_with(broken.as_bytes())?, &diff)
        .err()
        .ok_or("applied")?;
    assert_eq!(
        error,
        ApplyError::ContextMismatch {
            path: rel(SRC)?,
            hunk: 2,
            line: 27
        }
    );
    assert_eq!(
        error.reason(),
        Reason::ApplyFailed {
            path: rel(SRC)?,
            hunk: 2
        }
    );
    Ok(())
}

#[test]
fn apply_errors_display_without_content() -> TestResult {
    let diff = diff_of("SECRET\nb\n", "SECRET\nB\n")?;
    let error = apply(&tree_with(b"OTHER\nb\n")?, &diff)
        .err()
        .ok_or("applied")?;
    assert!(!error.to_string().contains("SECRET"));
    assert!(!format!("{error:?}").contains("SECRET"));
    assert!(!error.to_string().contains("OTHER"));
    Ok(())
}

#[test]
fn leaves_untouched_files_alone() -> TestResult {
    let base = tree_with(b"a\n")?;
    let out = apply(&base, &diff_of("a\n", "b\n")?)?;
    assert_eq!(out.get(&rel(LIB)?), base.get(&rel(LIB)?));
    assert_eq!(src_of(&out)?, b"b\n");
    Ok(())
}

proptest! {
    #[test]
    fn apply_reproduces_modified(base in file_text(), modified in file_text()) {
        prop_assume!(base != modified);
        let diff = diff_of(&base, &modified).map_err(fail)?;
        let out = applied(&base, &diff).map_err(fail)?;
        prop_assert_eq!(out, modified.into_bytes());
    }

    #[test]
    fn context_change_is_rejected(base in non_empty_text(), modified in file_text(), pick in any::<prop::sample::Index>(), byte in any::<u8>()) {
        prop_assume!(base != modified);
        let diff = diff_of(&base, &modified).map_err(fail)?;
        // Every base line the diff reads (context or delete), as a 0-based line index.
        let mut read = Vec::new();
        for hunk in diff.files().iter().flat_map(FileDiff::hunks) {
            let mut index = usize::try_from(hunk.old_start()).map_err(fail)?;
            if hunk.old_len() > 0 {
                index = index.saturating_sub(1);
            }
            for line in hunk.lines() {
                if !matches!(line, HunkLine::Insert(_)) {
                    read.push(index);
                    index += 1;
                }
            }
        }
        prop_assume!(!read.is_empty());
        let target = *pick.get(&read);
        let mut lines: Vec<Vec<u8>> = base.as_bytes().split_inclusive(|byte| *byte == b'\n').map(<[u8]>::to_vec).collect();
        let line = lines.get_mut(target).ok_or_else(|| fail("line index"))?;
        // Change one byte of the line (its newline counts), or append one if the line is empty.
        let at = pick.index(line.len().max(1));
        match line.get_mut(at) {
            Some(slot) if *slot != byte => *slot = byte,
            Some(slot) => *slot = byte.wrapping_add(1),
            None => line.push(byte),
        }
        let broken: Vec<u8> = lines.concat();
        prop_assume!(broken != base.as_bytes());
        let tree = tree_with(&broken).map_err(fail)?;
        match apply(&tree, &diff) {
            Ok(_) => prop_assert!(false, "a changed base line was accepted"),
            Err(ApplyError::ContextMismatch { hunk, line, .. }) => {
                prop_assert!(hunk >= 1);
                prop_assert!(line >= 1);
            }
            // A changed newline can split or join lines, which can move a hunk past the end.
            Err(ApplyError::OutOfRange { .. }) => {}
            Err(other) => prop_assert!(false, "unexpected error {:?}", other),
        }
    }

    #[test]
    fn apply_never_adds_or_removes_paths(base in file_text(), modified in file_text()) {
        prop_assume!(base != modified);
        let diff = diff_of(&base, &modified).map_err(fail)?;
        let tree = tree_with(base.as_bytes()).map_err(fail)?;
        let out = apply(&tree, &diff).map_err(fail)?;
        let before: Vec<&RelPath> = tree.iter().map(|(path, _)| path).collect();
        let after: Vec<&RelPath> = out.iter().map(|(path, _)| path).collect();
        prop_assert_eq!(before, after);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Task 2: composition and Experiment
// ---------------------------------------------------------------------------------------------------------------

/// The run's base for the composition tests: `tree` under one fixed tree ID.
fn based(tree: Tree) -> BaseTree {
    BaseTree::new(TreeId::of(b"base tarball"), tree)
}

fn compose_of(
    base: &BaseTree,
    parent: Option<&CumulativeDiff>,
    proposal: &Diff,
) -> Result<CumulativeDiff, ComposeError> {
    // Both literals are valid paths; a failed parse would shrink the set and fail the test, never pass it.
    let writable: BTreeSet<RelPath> = [SRC, LIB]
        .into_iter()
        .filter_map(|path| RelPath::try_from(path).ok())
        .collect();
    compose(base, parent, proposal, &writable, &Limits::standard())
}

fn diff_trees_of(base: &Tree, candidate: &Tree) -> Result<Diff, Box<dyn Error>> {
    Ok(diff_trees(
        base,
        candidate,
        &writable()?,
        &Limits::standard(),
    )?)
}

fn diff_trees_err(base: &Tree, candidate: &Tree) -> Result<ComposeError, Box<dyn Error>> {
    match diff_trees(base, candidate, &writable()?, &Limits::standard()) {
        Ok(_) => Err("diff_trees succeeded".into()),
        Err(error) => Ok(error),
    }
}

/// A hand-written proposal on [`SRC`] with the given hunks.
fn proposal(hunks: &str) -> Result<Diff, Box<dyn Error>> {
    parse(&format!(
        "diff --git a/{SRC} b/{SRC}\n--- a/{SRC}\n+++ b/{SRC}\n{hunks}"
    ))
}

#[test]
fn diff_trees_covers_several_files_in_path_order() -> TestResult {
    let mut base = tree_with(b"a\nb\n")?;
    base.insert(rel(LIB)?, b"one\ntwo\n".to_vec());
    let mut candidate = tree_with(b"a\nB\n")?;
    candidate.insert(rel(LIB)?, b"one\nTWO\n".to_vec());
    let diff = diff_trees_of(&base, &candidate)?;
    let paths: Vec<&str> = diff.paths().map(RelPath::as_str).collect();
    assert_eq!(paths, vec![LIB, SRC]);
    assert_eq!(apply(&base, &diff)?, candidate);
    Ok(())
}

#[test]
fn diff_trees_of_equal_trees_is_no_change() -> TestResult {
    let tree = tree_with(b"a\n")?;
    assert_eq!(diff_trees_err(&tree, &tree)?, ComposeError::NoChange);
    Ok(())
}

#[test]
fn diff_trees_rejects_a_changed_path_set() -> TestResult {
    let base = tree_with(b"a\n")?;
    let mut extra = base.clone();
    extra.insert(rel("src/new.rs")?, b"new\n".to_vec());
    assert_eq!(
        diff_trees_err(&base, &extra)?,
        ComposeError::PathSetChanged {
            path: rel("src/new.rs")?
        }
    );
    assert_eq!(
        diff_trees_err(&extra, &base)?,
        ComposeError::PathSetChanged {
            path: rel("src/new.rs")?
        }
    );
    let mut renamed = Tree::new();
    renamed.insert(rel(LIB)?, b"pub mod parse;\n".to_vec());
    renamed.insert(rel("src/parse2.rs")?, b"a\n".to_vec());
    assert_eq!(
        diff_trees_err(&base, &renamed)?,
        ComposeError::PathSetChanged { path: rel(SRC)? }
    );
    Ok(())
}

#[test]
fn diff_trees_rejects_non_text() -> TestResult {
    let base = tree_with(b"\xff\na\n")?;
    let candidate = tree_with(b"\xff\nb\n")?;
    assert_eq!(
        diff_trees_err(&base, &candidate)?,
        ComposeError::NotText { path: rel(SRC)? }
    );
    // The same through compose: the proposal touches only text lines, but the file is not UTF-8.
    let edit = proposal("@@ -2,1 +2,1 @@\n-a\n+b\n")?;
    assert_eq!(
        compose_of(&based(base), None, &edit),
        Err(ComposeError::NotText { path: rel(SRC)? })
    );
    Ok(())
}

#[test]
fn diff_trees_rejects_a_change_to_a_non_writable_file() -> TestResult {
    let mut base = tree_with(b"a\n")?;
    base.insert(rel("README.md")?, b"readme\n".to_vec());
    let mut candidate = base.clone();
    candidate.insert(rel("README.md")?, b"changed\n".to_vec());
    assert_eq!(
        diff_trees_err(&base, &candidate)?,
        ComposeError::Invalid(DiffError::NotWritable {
            path: rel("README.md")?
        })
    );
    Ok(())
}

#[test]
fn compose_reports_an_apply_failure() -> TestResult {
    let base = based(tree_with(b"a\nb\nc\n")?);
    let edit = proposal("@@ -1,1 +1,1 @@\n-x\n+y\n")?;
    assert_eq!(
        compose_of(&base, None, &edit),
        Err(ComposeError::Apply(ApplyError::ContextMismatch {
            path: rel(SRC)?,
            hunk: 1,
            line: 1
        }))
    );
    Ok(())
}

#[test]
fn reverting_the_parent_is_no_change() -> TestResult {
    let base = based(tree_with(b"a\nb\nc\n")?);
    let parent = compose_of(&base, None, &diff_of("a\nb\nc\n", "a\nB\nc\n")?)?;
    let revert = diff_of("a\nB\nc\n", "a\nb\nc\n")?;
    assert_eq!(
        compose_of(&base, Some(&parent), &revert),
        Err(ComposeError::NoChange)
    );
    Ok(())
}

#[test]
fn compose_is_cumulative_from_the_base() -> TestResult {
    let start = numbered(20);
    let base = based(tree_with(start.as_bytes())?);
    let first = start.replace("line 3\n", "line three\n");
    let parent = compose_of(&base, None, &diff_of(&start, &first)?)?;
    let second = first.replace("line 17\n", "line seventeen\n");
    let child = compose_of(&base, Some(&parent), &diff_of(&first, &second)?)?;
    // The child's diff carries both changes and applies to the base, not to the parent's tree.
    assert_eq!(
        src_of(&apply(base.tree(), child.as_diff())?)?,
        second.as_bytes()
    );
    assert_eq!(
        child
            .as_diff()
            .files()
            .first()
            .map(|file| file.hunks().len()),
        Some(2)
    );
    Ok(())
}

#[test]
fn same_tree_same_experiment_id() -> TestResult {
    let start = numbered(20);
    let base = based(tree_with(start.as_bytes())?);
    let target = start.replace("line 10\n", "line ten\n");
    // Three proposals with different bytes reach the same candidate tree.
    let wide = diff_of(&start, &target)?;
    let narrow = proposal("@@ -9,3 +9,3 @@\n line 9\n-line 10\n+line ten\n line 11\n")?;
    let bare = proposal("@@ -10,1 +10,1 @@\n-line 10\n+line ten\n")?;
    assert_ne!(wide.as_str(), narrow.as_str());
    assert_ne!(narrow.as_str(), bare.as_str());
    let parent = compose_of(
        &base,
        None,
        &proposal("@@ -10,1 +10,1 @@\n-line 10\n+draft\n")?,
    )?;
    let via_parent = proposal("@@ -10,1 +10,1 @@\n-draft\n+line ten\n")?;

    let tree_id = base.id();
    let composed = [
        compose_of(&base, None, &wide)?,
        compose_of(&base, None, &narrow)?,
        compose_of(&base, None, &bare)?,
        compose_of(&base, Some(&parent), &via_parent)?,
    ];
    let ids: BTreeSet<ExperimentId> = composed
        .iter()
        .map(|diff| ExperimentId::of(&tree_id, diff.as_diff()))
        .collect();
    assert_eq!(ids.len(), 1);
    let minted: BTreeSet<ExperimentId> = composed.iter().map(CumulativeDiff::exp).collect();
    assert_eq!(minted, ids);
    let texts: BTreeSet<&str> = composed
        .iter()
        .map(|diff| diff.as_diff().as_str())
        .collect();
    assert_eq!(texts.len(), 1);
    assert_eq!(texts.first().copied(), Some(wide.as_str()));
    Ok(())
}

#[test]
fn experiment_baseline_and_candidate() -> TestResult {
    let base = TreeId::of(b"base tarball");
    let baseline = Experiment::baseline(base);
    assert_eq!(baseline.id(), ExperimentId::baseline(&base));
    assert_eq!(baseline.base(), base);
    assert!(baseline.diff().is_none());
    assert!(baseline.is_baseline());

    let diff = diff_of("a\n", "b\n")?;
    let id = ExperimentId::of(&base, &diff);
    let candidate = Experiment::candidate(CumulativeDiff::recorded(base, diff.clone(), &id)?);
    assert_eq!(candidate.id(), ExperimentId::of(&base, &diff));
    assert_eq!(candidate.base(), base);
    assert_eq!(candidate.diff(), Some(&diff));
    assert!(!candidate.is_baseline());
    assert_ne!(candidate.id(), baseline.id());
    assert_ne!(candidate, baseline);
    Ok(())
}

#[test]
fn debug_omits_content() -> TestResult {
    let tree = tree_with(b"SECRET-CONTENT\n")?;
    let shown = format!("{tree:?}");
    assert!(!shown.contains("SECRET"));
    assert!(shown.contains(SRC));

    let diff = diff_of("SECRET-OLD\n", "SECRET-NEW\n")?;
    let tree = TreeId::of(b"base");
    let id = ExperimentId::of(&tree, &diff);
    let experiment = Experiment::candidate(CumulativeDiff::recorded(tree, diff, &id)?);
    let shown = format!("{experiment:?}");
    assert!(!shown.contains("SECRET"));
    assert!(shown.contains(&experiment.id().to_hex()));
    let baseline = format!("{:?}", Experiment::baseline(TreeId::of(b"base")));
    assert!(baseline.contains("Experiment"));
    Ok(())
}

proptest! {
    #[test]
    fn diff_trees_round_trip(base in file_text(), modified in file_text(), lib in file_text()) {
        let base_tree = tree_with(base.as_bytes()).map_err(fail)?;
        let mut candidate = tree_with(modified.as_bytes()).map_err(fail)?;
        candidate.insert(rel(LIB).map_err(fail)?, lib.into_bytes());
        prop_assume!(candidate != base_tree);
        let diff = diff_trees_of(&base_tree, &candidate).map_err(fail)?;
        prop_assert_eq!(apply(&base_tree, &diff).map_err(fail)?, candidate);
    }

    #[test]
    fn compose_matches_sequential_apply(a in file_text(), b in file_text(), c in file_text()) {
        prop_assume!(a != b);
        let base = based(tree_with(a.as_bytes()).map_err(fail)?);
        let p = diff_of(&a, &b).map_err(fail)?;
        let parent = compose_of(&base, None, &p).map_err(fail)?;
        prop_assert_eq!(
            apply(base.tree(), parent.as_diff()).map_err(fail)?,
            apply(base.tree(), &p).map_err(fail)?
        );
        // A proposal against the base is already canonical and cumulative.
        prop_assert_eq!(parent.as_diff().as_str(), p.as_str());

        prop_assume!(b != c);
        let q = diff_of(&b, &c).map_err(fail)?;
        let sequential = apply(&apply(base.tree(), parent.as_diff()).map_err(fail)?, &q).map_err(fail)?;
        match compose_of(&base, Some(&parent), &q) {
            Ok(child) => prop_assert_eq!(apply(base.tree(), child.as_diff()).map_err(fail)?, sequential),
            Err(ComposeError::NoChange) => prop_assert_eq!(&sequential, base.tree()),
            Err(other) => prop_assert!(false, "unexpected error {:?}", other),
        }
    }
}
