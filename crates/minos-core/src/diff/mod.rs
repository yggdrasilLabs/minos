//! The canonical `Diff` and its parse pipeline.
//!
//! `Diff::parse` enforces size caps, requires every touched path to be writable, and rejects renames, creates,
//! deletes, mode changes, symlinks, binary patches, and no-op diffs, so an experiment that edits the judge cannot
//! be represented. Filled by plan 03-02.

pub mod apply;

pub mod compose;
