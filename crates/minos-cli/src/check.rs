//! `minos check`: validate the project, refuse a missing or stale lock, image, or judge push naming the command to
//! run, run the provider preflight, judge the baseline several times on a diagnostic run (reusing a calibrated one
//! whose frame matches), and print the noise floor with advice, the route table with reasons, the memory budget,
//! and the judge-scratch requirement (CLI-05, JUDGE-02; 08-CONTEXT D-05 to D-09).
//!
//! Plan 08-07 fills this module.
