//! `BaseTree` and `CumulativeDiff`: a diff cumulative from the run's base as a type (07-CONTEXT D-12, 03-REVIEW
//! WR-03).
//!
//! The three minting paths are checked here: [`cumulative`] against the base handle, `compose` against the base
//! handle, and `CumulativeDiff::recorded` for a diff re-read under its recorded experiment ID. Every check also
//! pins that the bytes and the IDs are the ones the untyped functions gave, so the type changes no wire byte.

use std::collections::BTreeSet;
use std::error::Error;

use minos_core::Diff;
use minos_core::diff::apply::{Tree, apply};
use minos_core::diff::compose::{ComposeError, compose, diff_trees};
use minos_core::diff::cumulative::{BaseTree, CumulativeDiff, NotRecorded, cumulative};
use minos_core::experiment::Experiment;
use minos_core::id::{ExperimentId, TreeId};
use minos_core::limits::Limits;
use minos_core::path::{PathError, RelPath};

type TestResult = Result<(), Box<dyn Error>>;

/// The writable file the tests change.
const SRC: &str = "src/parse.rs";

/// A second writable file, changed only by the two-file case.
const LIB: &str = "src/lib.rs";

fn rel(path: &str) -> Result<RelPath, PathError> {
    RelPath::try_from(path)
}

fn writable() -> Result<BTreeSet<RelPath>, PathError> {
    [SRC, LIB].into_iter().map(rel).collect()
}

/// A tree with [`SRC`] holding `src` and [`LIB`] holding `lib`.
fn tree(src: &str, lib: &str) -> Result<Tree, PathError> {
    let mut tree = Tree::new();
    tree.insert(rel(SRC)?, src.as_bytes().to_vec());
    tree.insert(rel(LIB)?, lib.as_bytes().to_vec());
    Ok(tree)
}

/// The run's base: two files under a fixed tree ID.
fn base() -> Result<BaseTree, PathError> {
    Ok(BaseTree::new(
        TreeId::of(b"base tarball"),
        tree("fn parse() {}\nfn other() {}\n", "pub mod parse;\n")?,
    ))
}

/// The untyped canonical diff from `from` to `to`, as the old functions produced it.
fn untyped(from: &Tree, to: &Tree) -> Result<Diff, Box<dyn Error>> {
    Ok(diff_trees(from, to, &writable()?, &Limits::standard())?)
}

#[test]
fn cumulative_equals_diff_trees_against_the_base() -> TestResult {
    let base = base()?;
    let candidate = tree("fn parse() { fast() }\nfn other() {}\n", "pub mod parse;\n")?;
    let minted = cumulative(&base, &candidate, &writable()?, &Limits::standard())?;
    let expected = untyped(base.tree(), &candidate)?;
    assert_eq!(minted.as_diff(), &expected);
    assert_eq!(minted.as_diff().as_bytes(), expected.as_bytes());
    assert_eq!(minted.base(), base.id());
    assert_eq!(minted.exp(), ExperimentId::of(&base.id(), &expected));
    assert_eq!(minted.clone().into_diff(), expected);
    Ok(())
}

#[test]
fn cumulative_of_the_base_itself_is_no_change() -> TestResult {
    let base = base()?;
    let result = cumulative(&base, base.tree(), &writable()?, &Limits::standard());
    assert_eq!(result, Err(ComposeError::NoChange));
    Ok(())
}

#[test]
fn compose_without_a_parent_gives_the_untyped_bytes() -> TestResult {
    let base = base()?;
    let target = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let proposal = untyped(base.tree(), &target)?;
    let composed = compose(&base, None, &proposal, &writable()?, &Limits::standard())?;
    let expected = untyped(base.tree(), &apply(base.tree(), &proposal)?)?;
    assert_eq!(composed.as_diff().as_bytes(), expected.as_bytes());
    assert_eq!(composed.base(), base.id());
    Ok(())
}

#[test]
fn compose_with_a_parent_gives_the_untyped_bytes() -> TestResult {
    let base = base()?;
    let parent_tree = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let parent = cumulative(&base, &parent_tree, &writable()?, &Limits::standard())?;
    let child_tree = tree(
        "fn parse() { one() }\nfn other() {}\n",
        "pub mod parse;\npub mod two;\n",
    )?;
    // The proposal is written against the parent's state, as the model saw it.
    let proposal = untyped(&parent_tree, &child_tree)?;
    let composed = compose(
        &base,
        Some(&parent),
        &proposal,
        &writable()?,
        &Limits::standard(),
    )?;
    let sequential = apply(&apply(base.tree(), parent.as_diff())?, &proposal)?;
    let expected = untyped(base.tree(), &sequential)?;
    assert_eq!(composed.as_diff().as_bytes(), expected.as_bytes());
    assert_eq!(composed.base(), base.id());
    assert_eq!(apply(base.tree(), composed.as_diff())?, child_tree);
    Ok(())
}

#[test]
fn compose_refuses_a_parent_from_another_base() -> TestResult {
    let base = base()?;
    let other = BaseTree::new(TreeId::of(b"another tarball"), base.tree().clone());
    let parent_tree = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let foreign = cumulative(&other, &parent_tree, &writable()?, &Limits::standard())?;
    let child_tree = tree("fn parse() { two() }\nfn other() {}\n", "pub mod parse;\n")?;
    let proposal = untyped(&parent_tree, &child_tree)?;
    let result = compose(
        &base,
        Some(&foreign),
        &proposal,
        &writable()?,
        &Limits::standard(),
    );
    assert_eq!(result, Err(ComposeError::BaseMismatch));
    Ok(())
}

#[test]
fn recorded_accepts_a_diff_under_its_own_id() -> TestResult {
    let base = base()?;
    let target = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let diff = untyped(base.tree(), &target)?;
    let exp = ExperimentId::of(&base.id(), &diff);
    let re_read = CumulativeDiff::recorded(base.id(), diff.clone(), &exp)?;
    assert_eq!(re_read.as_diff(), &diff);
    assert_eq!(re_read.base(), base.id());
    assert_eq!(re_read.exp(), exp);
    Ok(())
}

#[test]
fn recorded_refuses_a_diff_under_another_id() -> TestResult {
    let base = base()?;
    let target = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let diff = untyped(base.tree(), &target)?;
    let baseline = ExperimentId::baseline(&base.id());
    let under_other_base = ExperimentId::of(&TreeId::of(b"another tarball"), &diff);
    for wrong in [baseline, under_other_base] {
        let refused = CumulativeDiff::recorded(base.id(), diff.clone(), &wrong);
        let error: NotRecorded = refused.err().ok_or("recorded accepted a wrong ID")?;
        assert_eq!(error.exp(), wrong);
        assert!(error.to_string().contains(&wrong.to_hex()));
    }
    Ok(())
}

#[test]
fn experiment_ids_are_unchanged() -> TestResult {
    let base = base()?;
    let target = tree("fn parse() { one() }\nfn other() {}\n", "pub mod parse;\n")?;
    let minted = cumulative(&base, &target, &writable()?, &Limits::standard())?;
    let expected_id = ExperimentId::of(&minted.base(), minted.as_diff());
    let experiment = Experiment::candidate(minted.clone());
    assert_eq!(experiment.id(), expected_id);
    assert_eq!(experiment.id(), minted.exp());
    assert_eq!(experiment.base(), base.id());
    assert_eq!(experiment.diff(), Some(minted.as_diff()));
    assert!(!experiment.is_baseline());
    Ok(())
}

#[test]
fn debug_omits_content() -> TestResult {
    let base = BaseTree::new(TreeId::of(b"base"), tree("SECRET-OLD\n", "SECRET-LIB\n")?);
    let candidate = tree("SECRET-NEW\n", "SECRET-LIB\n")?;
    let minted = cumulative(&base, &candidate, &writable()?, &Limits::standard())?;
    for shown in [format!("{base:?}"), format!("{minted:?}")] {
        assert!(!shown.contains("SECRET"), "{shown}");
        assert!(shown.contains(&base.id().to_hex()), "{shown}");
    }
    Ok(())
}
