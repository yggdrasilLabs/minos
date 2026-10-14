//! `Outcome`, `Reason`, and `Stage`: an experiment's failure is a value, never an error.
//!
//! An outcome is never retried; infrastructure failure is a separate error path. Filled by plan 03-03.
