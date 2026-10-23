//! The library half of the `minos` command-line interface: the parts the commands share and a backend is
//! constructed from, kept out of `main.rs` so they are testable and reusable (rules/proj-lib-main-split).
//!
//! - [`locate`] finds the `minos-vmm` helper, the static `minos-guest`, and libkrunfw at fixed locations, with no
//!   configuration file and no environment variable (05-CONTEXT D-04).
//! - [`builder`] is the container builder seam, [`builder::RootfsBuilder`], and its Docker buildx implementation,
//!   run as an argv array with a cleared environment (05-RESEARCH Pattern 10).
//! - [`env_build`] is the `minos env build` pipeline: snapshot, build, pack with `/init`, store, and `minos.lock`
//!   (KRUN-09, CLI-02; 05-CONTEXT D-09 to D-12).
//! - [`lock_file`] reads `minos.lock` under its cap and writes it through a synced temporary file in `.minos/`,
//!   shared by `env build` and `judge push` (05-CONTEXT D-10; 07-CONTEXT D-01).
//! - [`judge_push`] is the `minos judge push` pipeline: placement check, pack with the snapshot's hygiene, judge
//!   argv paths, store, and the `[[judge]]` lock entry under the manifest's label with the `--update` gate
//!   (JUDGE-01, CLI-04; 07-CONTEXT D-01 to D-04).
//! - [`blob_source`] implements [`minos_sandbox::BlobSource`] over the project's [`minos_store::FileStore`]: the
//!   CLI is the only crate where the store and the backend meet (05-CONTEXT D-13, D-14).
//!
//! The commands themselves, and `anyhow`, live in the binary (rules/err-anyhow-app).

pub mod blob_source;
pub mod builder;
pub mod env_build;
pub mod judge_push;
pub mod locate;
pub mod lock_file;
