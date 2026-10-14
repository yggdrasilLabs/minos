//! `Tree` and the strict exact-position applier: a hunk applies only at its stated line with exact context.
//!
//! A mismatch is a candidate fault, never an error, and the applier never searches other offsets.
//! Filled by plan 03-06.
