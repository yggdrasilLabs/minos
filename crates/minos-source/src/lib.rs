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
