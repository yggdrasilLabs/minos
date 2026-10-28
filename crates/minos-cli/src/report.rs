//! `minos report <exp>`: the hashed review report for an experiment, rebuilt from the ledger and blobs alone, with its
//! hash and the `minos adopt` command that accepts exactly that hash (CLI-08; 08-CONTEXT D-11).
//!
//! [`build`] names the run (the latest when `--run` is omitted) and the experiment by prefix, refuses the baseline,
//! and gathers what core's pure [`build_report`] is a function of:
//!
//! 1. the manifest in effect at the experiment's generation, read back from its recorded blob
//!    ([`recorded_manifest_at`]), so a later `ManifestChanged` never makes an earlier winner unreviewable;
//! 2. the run's one baseline judge version ([`run_judge`]);
//! 3. the base surface, rebuilt from the stored base tarball ([`base_surface`]), never from the working tree;
//! 4. the experiment's cumulative diff, re-read through [`recorded_diff`], which accepts it only when it hashes to
//!    the experiment;
//! 5. the blobs the report reads: the experiment's diff blob, read under the standard diff cap, and the rationale
//!    blob of every proposal in its lineage, read under [`MAX_RATIONALE_BYTES`], each keyed by [`BlobId::of`] its
//!    bytes.
//!
//! The same ledger and blobs always give the same report bytes and so the same [`ReportHash`]. [`report`] prints
//! every line of [`Report::text`] through [`write_line`], then `report hash: <64 hex>` and the `minos adopt` command
//! carrying that hash. The hash is the user's proof for `adopt`, which rebuilds the report with this same [`build`]
//! and refuses unless the hashes match (D-11).
//!
//! `report` reads only: it never appends to the ledger (there is no `report --ack`), never takes the run lock, reads
//! nothing from the working tree, and creates nothing.
//!
//! [`ReportHash`]: minos_core::id::ReportHash
//! [`recorded_manifest_at`]: minos_loop::history::recorded_manifest_at
//! [`recorded_diff`]: minos_loop::history::recorded_diff

use std::collections::BTreeMap;
use std::io::{self, Write};

use minos_core::diff::cumulative::CumulativeDiff;
use minos_core::id::{BlobId, ExperimentId, RunId};
use minos_core::limits::Limits;
use minos_core::report::{Report, ReportError, ReportInput, build_report};
use minos_loop::DriverError;
use minos_loop::history::{recorded_diff, recorded_manifest_at};
use minos_propose::limits::MAX_RATIONALE_BYTES;
use minos_store::{Blobs, Ledger, StoreError};

use crate::address::{AddressError, HexPrefix, fold_run, resolve_exp, resolve_run};
use crate::project::{PrepareError, base_surface};
use crate::render::{IdStyle, id_text, run_judge, write_line};

/// A report rebuilt from the ledger and blobs, with the run and experiment it covers and the experiment's recorded
/// cumulative diff, which `adopt` writes as the patch.
#[derive(Debug, Clone)]
pub struct BuiltReport {
    run: RunId,
    exp: ExperimentId,
    report: Report,
    diff: CumulativeDiff,
}

impl BuiltReport {
    /// The run.
    #[must_use]
    pub const fn run(&self) -> &RunId {
        &self.run
    }

    /// The experiment under review.
    #[must_use]
    pub const fn exp(&self) -> &ExperimentId {
        &self.exp
    }

    /// The report: its text and the hash of exactly that text.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// The experiment's recorded cumulative diff from the run's base, checked to hash to the experiment.
    #[must_use]
    pub const fn diff(&self) -> &CumulativeDiff {
        &self.diff
    }
}

/// Why a report could not be built or printed. Messages carry IDs and fixed text only, never blob bytes.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ReportCmdError {
    /// The run or the experiment could not be named, or the run's ledger does not fold.
    #[error("the run or experiment could not be named")]
    Address(#[source] AddressError),
    /// The experiment named is the baseline.
    #[error("the baseline has no diff to review; name a proposed experiment")]
    Baseline,
    /// The baseline's trials were not judged under one judge version.
    #[error("the run's baseline was not judged under exactly one judge version")]
    NoJudge,
    /// A diff or rationale blob could not be read under its cap.
    #[error("a recorded blob could not be read")]
    Store(#[source] StoreError),
    /// The run's base surface could not be rebuilt from its stored base tarball.
    #[error("the run's base could not be read back")]
    Prepare(#[source] Box<PrepareError>),
    /// The experiment's recorded diff or the manifest of its generation could not be read back and checked.
    #[error("the experiment's recorded diff or manifest could not be read back")]
    Record(#[source] Box<DriverError>),
    /// Core refused to build the report.
    #[error("the report could not be built")]
    Report(#[source] ReportError),
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Rebuilds the report for the experiment `exp` names, in the run `run` names in `store` or the latest run, from
/// the ledger and blobs alone, as the [module docs](self) give. Appends nothing.
///
/// # Errors
///
/// [`ReportCmdError::Address`] when the run or the experiment cannot be named or the run does not fold;
/// [`ReportCmdError::Baseline`] for the baseline; [`ReportCmdError::NoJudge`] when the baseline has no single judge
/// version; [`ReportCmdError::Record`] when the manifest of the experiment's generation or its diff cannot be read
/// back and checked; [`ReportCmdError::Prepare`] when the base cannot be rebuilt; [`ReportCmdError::Store`] when a
/// diff or rationale blob is missing, over its cap, or tampered; and [`ReportCmdError::Report`] when core refuses
/// the report (for example, an experiment with no complete score).
pub fn build<S: Ledger + Blobs>(
    store: &S,
    run: Option<&HexPrefix>,
    exp: &HexPrefix,
) -> Result<BuiltReport, ReportCmdError> {
    let resolved = resolve_run(store, run).map_err(ReportCmdError::Address)?;
    let state = fold_run(store, resolved.run()).map_err(ReportCmdError::Address)?;
    let exp = resolve_exp(&state, exp).map_err(ReportCmdError::Address)?;
    let proposal = state.proposal(&exp).ok_or(ReportCmdError::Baseline)?;
    let manifest = recorded_manifest_at(store, &state, proposal.generation()).map_err(record)?;
    let judge = run_judge(&state).ok_or(ReportCmdError::NoJudge)?;
    let base = base_surface(store, &state, &manifest)
        .map_err(|error| ReportCmdError::Prepare(Box::new(error)))?;
    let diff = recorded_diff(store, &state, &manifest, &base, &exp).map_err(record)?;

    let mut blobs = BTreeMap::new();
    let diff_bytes = blob(store, proposal.diff(), Limits::standard().max_diff_bytes())?;
    blobs.insert(BlobId::of(&diff_bytes), diff_bytes);
    for member in state.lineage(&exp).unwrap_or_default() {
        if let Some(member) = state.proposal(&member) {
            let bytes = blob(store, member.rationale(), MAX_RATIONALE_BYTES)?;
            blobs.insert(BlobId::of(&bytes), bytes);
        }
    }

    let report = build_report(&ReportInput::new(
        &state,
        &manifest,
        &judge,
        &exp,
        &blobs,
        base.tree(),
    ))
    .map_err(ReportCmdError::Report)?;
    Ok(BuiltReport {
        run: *state.run(),
        exp,
        report,
        diff,
    })
}

/// Rebuilds the report with [`build`] and writes it to `out`: every line of [`Report::text`], an empty line,
/// `report hash: <64 hex>`, and the `minos adopt` command that accepts exactly that hash. Appends nothing.
///
/// # Errors
///
/// As [`build`], and [`ReportCmdError::Output`] when `out` cannot be written.
pub fn report<S: Ledger + Blobs, W: Write>(
    store: &S,
    run: Option<&HexPrefix>,
    exp: &HexPrefix,
    out: &mut W,
) -> Result<BuiltReport, ReportCmdError> {
    let built = build(store, run, exp)?;
    for line in built.report.text().lines() {
        say(out, line)?;
    }
    say(out, "")?;
    let hash = built.report.hash().to_hex();
    say(out, &format!("report hash: {hash}"))?;
    say(
        out,
        &format!(
            "to adopt this experiment after review: minos adopt {} --run {} --reviewed {hash}",
            id_text(&built.exp.to_hex(), IdStyle::Short),
            id_text(&built.run.to_hex(), IdStyle::Short),
        ),
    )?;
    Ok(built)
}

/// The bytes of blob `id`, read under `max`.
fn blob<S: Blobs>(store: &S, id: &BlobId, max: usize) -> Result<Vec<u8>, ReportCmdError> {
    store
        .get(id, u64::try_from(max).unwrap_or(u64::MAX))
        .map_err(ReportCmdError::Store)
}

/// A driver error from re-reading a record, boxed.
fn record(error: DriverError) -> ReportCmdError {
    ReportCmdError::Record(Box::new(error))
}

/// Writes one line, mapping a write failure.
fn say<W: Write>(out: &mut W, line: &str) -> Result<(), ReportCmdError> {
    write_line(out, line).map_err(ReportCmdError::Output)
}
