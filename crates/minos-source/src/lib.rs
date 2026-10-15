//! The deterministic source snapshot of a Minos project.
//!
//! This crate walks a project tree under four exclusion sources (the project's `.gitignore` files,
//! its `.minosignore` files, the manifest's `[source] exclude` entries, and a hard denylist), refuses
//! anything that is not a regular file whose path fits the core path grammar, scans the exact bytes it
//! will pack for secrets, checks that the tree is committed, and writes a deterministic ustar tarball
//! (sorted paths, zeroed metadata, regular files only) whose SHA-256 is the snapshot's `TreeId`.
//!
//! The `snapshot` entry point arrives in plan 04-07. Only `minos-cli` links this crate: `deny.toml`
//! refuses any other dependent, so the filesystem walker and the read-only `git` exec of the clean
//! check never enter the loop's or the runner's graph.
//!
//! The guards are public and tested on their own: [`denylist`], [`scan`], and [`clean`]. None of them produces a
//! tarball, so exposing them opens no path around the scan.

pub mod clean;
pub mod denylist;
pub mod scan;

pub use clean::{CleanPolicy, Dirty, check_clean};
pub use denylist::{DenyRule, deny_component};
pub use scan::{ScanError, Scanner, SecretHit};
