//! `RawManifest` (the mirror of `minos.toml`) and the validated `Manifest` built from it.
//!
//! `Manifest::try_from` adds every cross-field check: the surface is non-empty and disjoint from readable and judge
//! paths, budgets are within `limits`, images are digests, and the proposer ladder has an eligible tier.
//! Filled by plan 03-05.
