//! The library half of the `minos` command-line interface: the parts the commands share and a backend is
//! constructed from, kept out of `main.rs` so they are testable and reusable (rules/proj-lib-main-split).
//!
//! - [`locate`] finds the `minos-vmm` helper, the static `minos-guest`, and libkrunfw at fixed locations, with no
//!   configuration file and no environment variable (05-CONTEXT D-04).
//! - [`blob_source`] implements [`minos_sandbox::BlobSource`] over the project's [`minos_store::FileStore`]: the
//!   CLI is the only crate where the store and the backend meet (05-CONTEXT D-13, D-14).
//!
//! The commands themselves, and `anyhow`, live in the binary (rules/err-anyhow-app).

pub mod blob_source;
pub mod locate;
