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
//! - [`jobs`] adapts the production [`minos_runner::Runner`] to the loop's [`minos_loop::JobRunner`] seam, mapping
//!   each runner error onto a job failure by its blame, and reads the judge inputs from `minos.lock`: the CLI is the
//!   only crate where the runner and the loop meet (07-CONTEXT D-02, D-16).
//! - [`project`] prepares a project for a run, in the order `check` and `run` share, with refusals that name the
//!   next command; opens the store for the read-only commands without creating anything; and rebuilds a run's base
//!   surface from its stored tarball (08-CONTEXT D-06).
//! - [`address`] names runs and experiments by unique lowercase hex prefix, and picks the latest run when none is
//!   named (08-CONTEXT D-10).
//! - [`render`] holds the output helpers every command shares: short IDs, closed-set words, percents, times, the
//!   cost line, and [`render::write_line`], which passes every printed line through `SafeText` (08-CONTEXT D-14 to
//!   D-17).
//! - [`check`] and [`run`] drive the loop: calibration with its advice, and a run to an absolute generation
//!   (CLI-05, CLI-06).
//! - [`status`], [`log`], and [`show`] read a run's ledger and blobs (CLI-07).
//! - [`report`] and [`adopt`] are the review gate: the hashed report, its acknowledgement, and the patch file
//!   (CLI-08, CLI-09).
//! - [`stop`] asks a running driver to stop (CLI-10), and [`init`] scaffolds a project and a price table (CLI-01).
//!
//! The commands are library functions that write to a caller's writer. `main.rs` parses arguments, picks the concrete
//! provider and backend, and maps outcomes to exit codes; `anyhow` stays in the binary (rules/err-anyhow-app).

pub mod address;
pub mod adopt;
pub mod blob_source;
pub mod builder;
pub mod check;
pub mod env_build;
pub mod init;
pub mod jobs;
pub mod judge_push;
pub mod locate;
pub mod lock_file;
pub mod log;
pub mod project;
pub mod render;
pub mod report;
pub mod run;
pub mod show;
pub mod status;
pub mod stop;
