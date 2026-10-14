//! `Experiment`: a base tree plus a parsed cumulative `Diff`, or the baseline itself.
//!
//! An experiment can only be built from a parsed diff, so it can never touch a non-writable path.
//! Filled by plan 03-06.
