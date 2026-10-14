//! `Metric` (a finite `f64` ordered with `total_cmp`) and `Scorecard`, the judge's validated output.
//!
//! A candidate can influence the numbers a judge emits, so no NaN or infinity ever reaches policy code.
//! Filled by plan 03-03.
