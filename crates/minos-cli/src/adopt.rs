//! `minos adopt <exp> --reviewed <hash>`: the review gate's last step. It writes the experiment's cumulative diff
//! as a patch file only after the user names the hash of the report as it stands now, and it never applies the
//! patch (CLI-09; 08-CONTEXT D-11, D-12).
//!
//! [`adopt`] takes these steps in order, and each refusal stops it before the next:
//!
//! 1. It rebuilds the report from the ledger and blobs now, with the same [`report::build`] that `minos report`
//!    prints, so the report is the one a user would read at this moment.
//! 2. It compares that report's [`ReportHash`] with `--reviewed`. A different hash, including the hash of a report
//!    that changed after the user read it (a new trial, a re-score), is [`AdoptError::HashMismatch`]: nothing is
//!    appended and nothing is written.
//! 3. It appends `ReportAcknowledged { exp, report }`, so the ledger records the acknowledgment at the moment it
//!    gates the write. Its key is the experiment and the hash, so a repeated `adopt` is a `Duplicate` append,
//!    reported as already recorded; any store error, a `Conflict` included, stops it before any file is created.
//! 4. It creates the patch file, `<project>/minos-<first 12 hex of the experiment>.patch` or `--out <path>`, with
//!    `create_new`, which refuses an existing file and a symlink at the path alike ([`AdoptError::Exists`]): `adopt`
//!    never overwrites a file and never follows a link. The file holds exactly the canonical cumulative diff bytes
//!    the experiment ID hashes, and is synced before `adopt` reports it; a write that fails removes the file it
//!    created.
//! 5. It prints the path, the patch through [`write_line`] (the on-screen copy passes `SafeText`, while the file
//!    keeps the canonical bytes, because altering them would make the hash lie), and the two commands that apply
//!    it from the project directory: `patch -p1 -i <file>`, and `git apply` with `--directory=` set to the
//!    project's prefix inside its repository, with the note that plain `git apply` run in a subdirectory of a
//!    repository skips every file and still exits 0 (08-RESEARCH Pitfall 1).
//!
//! Minos never applies a patch: `adopt` writes one file, execs nothing, and never writes into the working tree.
//! The printed commands are text for the user, never run.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use minos_core::id::{ExperimentId, ReportHash};
use minos_core::ledger::{Event, Record};
use minos_core::text::SafeText;
use minos_store::{Appended, Blobs, Ledger, StoreError};

use crate::address::HexPrefix;
use crate::render::{IdStyle, id_text, write_line};
use crate::report::{self, ReportCmdError};

/// What `adopt` was asked to accept and where it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptOptions {
    reviewed: ReportHash,
    out: Option<PathBuf>,
}

impl AdoptOptions {
    /// Options that accept only the report hash `reviewed` (`--reviewed`) and write the patch to `out` (`--out`),
    /// or to the project's default patch path when `None`.
    #[must_use]
    pub const fn new(reviewed: ReportHash, out: Option<PathBuf>) -> Self {
        Self { reviewed, out }
    }
}

/// What `adopt` did: where it wrote the patch, and whether this call recorded the acknowledgment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptOutcome {
    path: PathBuf,
    newly_acknowledged: bool,
}

impl AdoptOutcome {
    /// The patch file written.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether this call appended the acknowledgment; `false` when an equal one was already recorded.
    #[must_use]
    pub const fn newly_acknowledged(&self) -> bool {
        self.newly_acknowledged
    }
}

/// Why `adopt` refused or failed. Messages carry IDs, hashes, a sanitized path, and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum AdoptError {
    /// The report could not be rebuilt.
    #[error(transparent)]
    Report(ReportCmdError),
    /// The report as it stands now does not hash to the hash given.
    #[error(
        "the report for {exp} now hashes to {current}, not the hash given; run minos report and review it again"
    )]
    HashMismatch {
        /// The experiment.
        exp: ExperimentId,
        /// The hash of the report as rebuilt now.
        current: ReportHash,
    },
    /// Something already exists at the patch path: a file, a directory, or a symlink.
    #[error(
        "{path} already exists; adopt never overwrites a file; remove it or pass --out with a new path"
    )]
    Exists {
        /// The path, sanitized.
        path: SafeText,
    },
    /// The acknowledgment could not be appended.
    #[error("the acknowledgment could not be recorded")]
    Store(#[source] StoreError),
    /// The patch file could not be created or written.
    #[error("the patch file could not be written")]
    Write {
        /// Why.
        #[source]
        source: io::Error,
    },
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Accepts the report for the experiment `exp` names, in the run `run` names in `store` or the latest run, when
/// `options` names its current hash, records the acknowledgment, and writes the patch file under `project` or at
/// `--out`, in the order the [module docs](self) give. It never applies the patch.
///
/// # Errors
///
/// [`AdoptError::Report`] when the report cannot be rebuilt (see [`report::build`]);
/// [`AdoptError::HashMismatch`] when the report now hashes to anything but the hash given, before any append;
/// [`AdoptError::Store`] when the acknowledgment cannot be appended, before any file is created;
/// [`AdoptError::Exists`] when anything exists at the patch path; [`AdoptError::Write`] when the patch file cannot
/// be created, written, or synced; and [`AdoptError::Output`] when `out` cannot be written.
pub fn adopt<S: Ledger + Blobs, W: Write>(
    store: &S,
    project: &Path,
    run: Option<&HexPrefix>,
    exp: &HexPrefix,
    options: AdoptOptions,
    out: &mut W,
) -> Result<AdoptOutcome, AdoptError> {
    let AdoptOptions {
        reviewed,
        out: path,
    } = options;
    let built = report::build(store, run, exp).map_err(AdoptError::Report)?;
    let exp = *built.exp();
    let current = *built.report().hash();
    if current != reviewed {
        return Err(AdoptError::HashMismatch { exp, current });
    }

    let appended = store
        .append(
            built.run(),
            Record::v1(Event::ReportAcknowledged {
                exp,
                report: current,
            }),
        )
        .map_err(AdoptError::Store)?;
    let newly_acknowledged = matches!(appended, Appended::New { .. });
    let short_exp = id_text(&exp.to_hex(), IdStyle::Short);
    say(
        out,
        &format!(
            "acknowledged report {} for experiment {short_exp} ({})",
            id_text(&current.to_hex(), IdStyle::Short),
            if newly_acknowledged {
                "recorded"
            } else {
                "already recorded"
            },
        ),
    )?;

    let in_project = path.is_none();
    let path = path.unwrap_or_else(|| project.join(format!("minos-{short_exp}.patch")));
    write_patch(&path, built.diff().as_diff().as_bytes())?;

    let shown = path.display().to_string();
    let file = if in_project {
        path.file_name()
            .map_or_else(|| shown.clone(), |name| name.to_string_lossy().into_owned())
    } else {
        shown.clone()
    };
    say(out, &format!("patch: {}", SafeText::inline(&shown)))?;
    say(out, "patch contents:")?;
    let text = SafeText::from_utf8_lossy(built.diff().as_diff().as_bytes());
    for line in text.as_str().lines() {
        say(out, line)?;
    }
    let file = SafeText::inline(&file);
    say(out, "apply it from the project directory with either:")?;
    say(out, &format!("  patch -p1 -i {file}"))?;
    say(
        out,
        &format!("  git apply --directory=\"$(git rev-parse --show-prefix)\" {file}"),
    )?;
    say(
        out,
        "plain git apply run inside a subdirectory of a repository skips every file and still exits 0; \
         minos never applies a patch",
    )?;
    Ok(AdoptOutcome {
        path,
        newly_acknowledged,
    })
}

/// Creates `path` with `create_new` (so an existing file, directory, or symlink is refused and never followed),
/// writes `bytes`, and syncs it. A write or sync that fails removes the file this call created.
fn write_patch(path: &Path, bytes: &[u8]) -> Result<(), AdoptError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                AdoptError::Exists {
                    path: SafeText::inline(&path.display().to_string()),
                }
            } else {
                AdoptError::Write { source }
            }
        })?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(source) = written {
        drop(file);
        // The file is this call's own (`create_new` made it), so a partial patch never stays behind. When the
        // removal also fails, the write error is still the one reported.
        fs::remove_file(path).ok();
        return Err(AdoptError::Write { source });
    }
    Ok(())
}

/// Writes one line, mapping a write failure.
fn say<W: Write>(out: &mut W, line: &str) -> Result<(), AdoptError> {
    write_line(out, line).map_err(AdoptError::Output)
}
