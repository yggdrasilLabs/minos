//! `minos init --kind <tune|optimize|explore>`: scaffold `minos.toml`, `program.md`, `minos.Dockerfile`, and the
//! project's `.gitignore`, and write a `prices.toml` template when none exists (CLI-01).
//!
//! The scaffold generalizes the shipped sample, `examples/logparse`: the same stage environment and digest-pinned
//! Dockerfile, the same judge layout (a judge crate and its private data in `judge/`, excluded from the source
//! snapshot and pushed with `minos judge push judge`), and a greedy policy over a small batch. Each kind's
//! manifest parses with [`Manifest::parse_toml`] and its Dockerfile passes [`base_images`]; the gate tests both.
//! The Dockerfile names its base image by digest alone, with the tag in a comment, because `minos env build`
//! refuses a tag beside a digest.
//!
//! Nothing is ever overwritten. [`init`] checks all four scaffold paths with `symlink_metadata` first, so a file,
//! a directory, or a symlink (dangling or not) at any of them refuses the whole scaffold with [`InitError::Exists`]
//! naming every present path, and nothing is written. Each file is then created with `create_new` (`O_EXCL`), so
//! a file that appears in between is refused too and a symlink is never followed; a write that fails removes the
//! files this call created.
//!
//! The price table is written only when nothing is at its path, with missing parents created mode 0700 and the
//! file mode 0600. [`PRICES_TEMPLATE`] fails closed: its compute rates are non-zero (AWS Lambda `MicroVM` list
//! rates, so a local run is never priced at zero) and its `[models]` table is empty, so a run whose proposer names
//! a model is refused with `PricesMissing` until the user prices that model. Every rate is a string.
//!
//! [`Manifest::parse_toml`]: minos_core::manifest::Manifest::parse_toml
//! [`base_images`]: minos_core::image::base_images

use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use minos_core::manifest::ProjectKind;
use minos_core::text::SafeText;

/// The kind of project to scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InitKind {
    /// Tuning declared knobs with the sweep tier first.
    Tune,
    /// Optimizing a measured objective.
    Optimize,
    /// An open-ended design question.
    Explore,
}

impl InitKind {
    /// The project kind the scaffolded manifest names.
    #[must_use]
    pub const fn project_kind(self) -> ProjectKind {
        match self {
            Self::Tune => ProjectKind::Tune,
            Self::Optimize => ProjectKind::Optimize,
            Self::Explore => ProjectKind::Explore,
        }
    }

    /// The kind's name, as `--kind` takes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tune => "tune",
            Self::Optimize => "optimize",
            Self::Explore => "explore",
        }
    }
}

impl fmt::Display for InitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A `--kind` that names no project kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind} is not a project kind; use tune, optimize, or explore")]
pub struct UnknownKind {
    kind: SafeText,
}

impl FromStr for InitKind {
    type Err = UnknownKind;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "tune" => Ok(Self::Tune),
            "optimize" => Ok(Self::Optimize),
            "explore" => Ok(Self::Explore),
            _ => Err(UnknownKind {
                kind: SafeText::inline(text),
            }),
        }
    }
}

/// What `init` was asked to scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitOptions {
    kind: InitKind,
}

impl InitOptions {
    /// Options that scaffold a project of `kind` (`--kind`).
    #[must_use]
    pub const fn new(kind: InitKind) -> Self {
        Self { kind }
    }

    /// The kind to scaffold.
    #[must_use]
    pub const fn kind(&self) -> InitKind {
        self.kind
    }
}

/// What `init` did with the price table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PricesFile {
    /// Nothing was at the path, so [`PRICES_TEMPLATE`] was written there.
    Written(PathBuf),
    /// Something was at the path, so it was kept unchanged.
    Kept(PathBuf),
}

/// What `init` wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    written: Vec<PathBuf>,
    prices: PricesFile,
}

impl InitReport {
    /// The scaffold files written, in the order `minos.toml`, `program.md`, `minos.Dockerfile`, `.gitignore`.
    #[must_use]
    pub fn written(&self) -> &[PathBuf] {
        &self.written
    }

    /// What happened to the price table.
    #[must_use]
    pub const fn prices(&self) -> &PricesFile {
        &self.prices
    }
}

/// Why `init` refused or failed. Messages carry sanitized paths and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    /// At least one scaffold path is taken, so nothing was written.
    #[error("{} already exist; init never overwrites a file", join_paths(paths))]
    Exists {
        /// Every scaffold path that is present, sanitized, in scaffold order.
        paths: Vec<SafeText>,
    },
    /// A file or directory could not be checked, created, or written.
    #[error("writing {what} failed")]
    Write {
        /// What was being written.
        what: &'static str,
        /// Why.
        #[source]
        source: io::Error,
    },
}

/// The paths joined with `, `.
fn join_paths(paths: &[SafeText]) -> String {
    paths
        .iter()
        .map(SafeText::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------------------------------------------
// The templates
// ---------------------------------------------------------------------------------------------------------------

/// The `optimize` manifest: the sample project generalized.
const OPTIMIZE_MANIFEST: &str = r#"# A Minos project: improve one source file against a judge that scores what the program writes.
# `minos check` validates this file, builds the image, and judges the baseline before any run.
#
# Before `minos check`: create the writable and readable files named below, put the judge crate and its private
# data in `judge/`, run `minos env build`, and push the judge with `minos judge push judge`.

[project]
# A lowercase letter, then lowercase letters, digits, and _.
name = "my_project"
kind = "optimize"
# Your instructions to the proposer.
program = "program.md"

[surface]
# The files a proposer may change (single files, never a directory or a glob), and the context it sees beside program.md.
# Every one must exist before `minos check`.
writable = ["src/solution.rs"]
readable = ["src/main.rs"]

[source]
# Kept out of the source snapshot and the image build context. `judge/` holds the judge crate and its private
# data, so no candidate and no proposer ever sees it.
exclude = ["judge/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }
# memory_mib and scratch_mib default to 2048 and 1024.

[commands]
# Argv arrays, never a shell string. The VM has no network, so the build runs offline with --frozen against a
# committed Cargo.lock. The program writes its output under /out.
build = ["cargo", "build", "--release", "--frozen"]
run = ["./target/release/app", "/out/result.txt"]

[budget]
# Stage time limits, and the spend cap in US dollars. Spend is metered and priced from prices.toml.
build = "5m"
run = "1m"
judge = "5m"
spend_usd = 5

[judge]
version = 1
# The judge VM unpacks the pushed `judge/` directory into its working directory, which is read-only, so cargo
# builds into /tmp. It reads the candidate's output at /scratch/candidate/out/result.txt and writes its scorecard
# to /out/scorecard.json.
command = [
  "cargo", "run", "--release", "--frozen", "--quiet", "--target-dir", "/tmp/judge-target",
  "--", "expected.txt", "/scratch/candidate/out/result.txt", "/out/scorecard.json",
]

[[judge.gate]]
# A gate passes only when its metric is 1.
metric = "output_valid"

[[judge.objective]]
metric = "score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 2

[[proposer]]
# One model call with no tools, returning `batch` proposals. Price the model in prices.toml.
kind = "single_shot"
model = "haiku"
batch = 2
"#;

/// The `tune` manifest: one integer knob swept first, then a model.
const TUNE_MANIFEST: &str = r#"# A Minos project: tune declared knobs against a judge that scores what the program writes.
# `minos check` validates this file, builds the image, and judges the baseline before any run.
#
# Before `minos check`: create the writable and readable files named below, put the judge crate and its private
# data in `judge/`, run `minos env build`, and push the judge with `minos judge push judge`.

[project]
# A lowercase letter, then lowercase letters, digits, and _.
name = "my_project"
kind = "tune"
# Your instructions to the proposer.
program = "program.md"

[surface]
# The files a proposer may change (single files, never a directory or a glob), and the context it sees beside program.md.
# Every one must exist before `minos check`.
writable = ["config/params.txt"]
readable = ["src/main.rs"]

[source]
# Kept out of the source snapshot and the image build context. `judge/` holds the judge crate and its private
# data, so no candidate and no proposer ever sees it.
exclude = ["judge/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }
# memory_mib and scratch_mib default to 2048 and 1024.

[commands]
# Argv arrays, never a shell string. The VM has no network, so the build runs offline with --frozen against a
# committed Cargo.lock. The program reads its knobs and writes its output under /out.
build = ["cargo", "build", "--release", "--frozen"]
run = ["./target/release/app", "config/params.txt", "/out/result.txt"]

[budget]
# Stage time limits, and the spend cap in US dollars. Spend is metered and priced from prices.toml.
build = "5m"
run = "1m"
judge = "5m"
spend_usd = 5

[judge]
version = 1
# The judge VM unpacks the pushed `judge/` directory into its working directory, which is read-only, so cargo
# builds into /tmp. It reads the candidate's output at /scratch/candidate/out/result.txt and writes its scorecard
# to /out/scorecard.json.
command = [
  "cargo", "run", "--release", "--frozen", "--quiet", "--target-dir", "/tmp/judge-target",
  "--", "expected.txt", "/scratch/candidate/out/result.txt", "/out/scorecard.json",
]

[[judge.gate]]
# A gate passes only when its metric is 1.
metric = "output_valid"

[[judge.objective]]
metric = "score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 2

[[param]]
# A knob the sweep tier samples: it rewrites the rest of the line after `anchor` in `file`, a writable file.
# Integer ranges take min, max, and step; string knobs take choices = ["a", "b"] instead.
name = "batch_size"
file = "config/params.txt"
anchor = "batch_size = "
min = 8
max = 256
step = 8

[[proposer]]
# Seeded sampling over the params above; no model.
kind = "sweep"
batch = 4

[[proposer]]
# After the sweep stalls, one model call with no tools, returning `batch` proposals. Price the model in
# prices.toml.
kind = "single_shot"
model = "haiku"
batch = 2
"#;

/// The `explore` manifest: the optimize layout around an open question.
const EXPLORE_MANIFEST: &str = r#"# A Minos project: explore an open design question against a judge that scores what the program writes.
# `minos check` validates this file, builds the image, and judges the baseline before any run.
#
# Before `minos check`: create the writable and readable files named below, put the judge crate and its private
# data in `judge/`, run `minos env build`, and push the judge with `minos judge push judge`.

[project]
# A lowercase letter, then lowercase letters, digits, and _.
name = "my_project"
kind = "explore"
# Your instructions to the proposer.
program = "program.md"

[surface]
# The files a proposer may change (single files, never a directory or a glob), and the context it sees beside program.md.
# Every one must exist before `minos check`.
writable = ["src/solution.rs"]
readable = ["src/main.rs"]

[source]
# Kept out of the source snapshot and the image build context. `judge/` holds the judge crate and its private
# data, so no candidate and no proposer ever sees it.
exclude = ["judge/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }
# memory_mib and scratch_mib default to 2048 and 1024.

[commands]
# Argv arrays, never a shell string. The VM has no network, so the build runs offline with --frozen against a
# committed Cargo.lock. The program writes its output under /out.
build = ["cargo", "build", "--release", "--frozen"]
run = ["./target/release/app", "/out/result.txt"]

[budget]
# Stage time limits, and the spend cap in US dollars. Spend is metered and priced from prices.toml.
build = "5m"
run = "1m"
judge = "5m"
spend_usd = 5

[judge]
version = 1
# The judge VM unpacks the pushed `judge/` directory into its working directory, which is read-only, so cargo
# builds into /tmp. It reads the candidate's output at /scratch/candidate/out/result.txt and writes its scorecard
# to /out/scorecard.json.
command = [
  "cargo", "run", "--release", "--frozen", "--quiet", "--target-dir", "/tmp/judge-target",
  "--", "expected.txt", "/scratch/candidate/out/result.txt", "/out/scorecard.json",
]

[[judge.gate]]
# A gate passes only when its metric is 1.
metric = "output_valid"

[[judge.objective]]
metric = "score"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 2

[[proposer]]
# One model call with no tools, returning `batch` different answers to the question. Price the model in
# prices.toml.
kind = "single_shot"
model = "haiku"
batch = 4
"#;

/// The `optimize` program.
const OPTIMIZE_PROGRAM: &str = r"# Task: <what to improve>

`src/solution.rs` is the one file you may change; `src/main.rs` is shown for context. The program writes its
result to `/out/result.txt`, and a judge you cannot see scores it.

## What a better result is

<Describe the output the judge rewards, with a worked example of an input and the output it should produce.>

## Rules

- Keep the public functions `src/main.rs` calls, with their signatures.
- Std only: add no dependencies.
- Do not hard-code answers to visible examples; scoring also covers inputs you are not shown.
";

/// The `tune` program.
const TUNE_PROGRAM: &str = r"# Task: <what to tune>

`config/params.txt` is the one file you may change; `src/main.rs` reads it and is shown for context. The program
writes its result to `/out/result.txt`, and a judge you cannot see scores it.

## The knobs

- `batch_size = <n>`: <what it controls>. Values from 8 to 256 in steps of 8.

<Describe any other knob in the file, and what a better result is.>

## Rules

- Keep one `name = value` line per knob, in the same form.
";

/// The `explore` program.
const EXPLORE_PROGRAM: &str = r"# Question: <the open question to explore>

`src/solution.rs` is the one file you may change; `src/main.rs` is shown for context. Each proposal is one
answer to the question, written as code. The program writes its result to `/out/result.txt`, and a judge you
cannot see scores it.

## What a good answer looks like

<Describe the qualities the judge rewards, and the approaches already tried, so proposals differ from them and
from each other.>

## Rules

- Keep the public functions `src/main.rs` calls, with their signatures.
- Std only: add no dependencies.
";

/// The manifest template for `kind`.
#[must_use]
pub const fn manifest_template(kind: InitKind) -> &'static str {
    match kind {
        InitKind::Tune => TUNE_MANIFEST,
        InitKind::Optimize => OPTIMIZE_MANIFEST,
        InitKind::Explore => EXPLORE_MANIFEST,
    }
}

/// The `program.md` template for `kind`.
#[must_use]
pub const fn program_template(kind: InitKind) -> &'static str {
    match kind {
        InitKind::Tune => TUNE_PROGRAM,
        InitKind::Optimize => OPTIMIZE_PROGRAM,
        InitKind::Explore => EXPLORE_PROGRAM,
    }
}

/// The scaffold's `minos.Dockerfile`: the sample's, with the base image pinned by its digest alone.
pub const DOCKERFILE_TEMPLATE: &str = r"# The build environment for this project.
#
# Minos runs every stage (build, run, judge) as uid 64000 with exactly these four variables:
#   PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
#   HOME=/scratch/home
#   TMPDIR=/tmp
#   LANG=C
# ENV lines in this file never reach a stage, and USER is ignored. A toolchain must therefore be reachable on that
# PATH and readable by any user.
#
# The base image is rust:1.97.1-slim-trixie, pinned by its index digest; buildx resolves the linux/arm64 image
# from it. Minos refuses a tag beside the digest, because the digest must be the only identity of the image.
FROM rust@sha256:8e8cf8f7fd54a2d23d5a743b3a03f56e26b6c774276c33fa0595111704ebb15c

# The image's cargo and rustc are rustup proxies in /usr/local/cargo/bin, which is not on the stage PATH, and rustup
# would look for its toolchains under HOME. Link the toolchain's own binaries into /usr/local/bin instead, so no
# rustup proxy ever runs in a stage. rustc finds its sysroot through the resolved link.
RUN ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/cargo /usr/local/bin/cargo \
 && ln -s /usr/local/rustup/toolchains/1.97.1-aarch64-unknown-linux-gnu/bin/rustc /usr/local/bin/rustc
";

/// The scaffold's `.gitignore`: what Minos writes into the project stays out of git, so the clean check passes.
pub const GITIGNORE_TEMPLATE: &str = r"# Files Minos writes into this project. They stay out of git so the clean check passes after
# `minos env build`, `minos judge push`, and `minos adopt`.
/.minos/
/target/
/judge/target/
/minos-*.patch
# minos.lock is not listed: commit it. It records the image and judge data this project runs against.
";

/// The starter `prices.toml`, written by `init` only when none exists. It fails closed: compute is priced above
/// zero and no model is priced, so a model proposer is refused until its rates are added.
pub const PRICES_TEMPLATE: &str = r#"# Minos's price table. The ledger records usage only; dollars are derived from these rates.
# Every rate is a string, in US dollars per unit.
v = 1

# Local VMs cost you electricity, not these. These are AWS Lambda MicroVM list rates, used so a local run is never
# priced at zero.
[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

# Price every model a [[proposer]] names and every resolved model ID the claude CLI reports (for example
# "claude-haiku-4-5"). A run whose proposer names an unpriced model is refused before any call, and a call that
# reports an unpriced model ends the run, so no model is ever priced at zero.
[models]
# [models."haiku"]
# input_token = "<dollars per input token>"
# output_token = "<dollars per output token>"
# cache_creation_input_token = "<dollars per cache-write token>"
# cache_read_input_token = "<dollars per cache-read token>"
"#;

/// The scaffold: each file's name, its contents for a kind, and what it is called in an error.
const SCAFFOLD: [(&str, &str); 4] = [
    ("minos.toml", "the manifest"),
    ("program.md", "the program"),
    ("minos.Dockerfile", "the Dockerfile"),
    (".gitignore", "the .gitignore"),
];

/// The contents of scaffold file `index` for `kind`.
const fn scaffold_contents(index: usize, kind: InitKind) -> &'static str {
    match index {
        0 => manifest_template(kind),
        1 => program_template(kind),
        2 => DOCKERFILE_TEMPLATE,
        _ => GITIGNORE_TEMPLATE,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------------------------------------------

/// Scaffolds a project of `options`' kind in `project`, and writes [`PRICES_TEMPLATE`] at `prices` when nothing is
/// there. It never overwrites a file and never writes through a symlink.
///
/// # Errors
///
/// [`InitError::Exists`] naming every present scaffold path, with nothing written, when any of the four is taken
/// (a file, a directory, or a symlink); and [`InitError::Write`] when a path cannot be checked or a file or
/// directory cannot be created or written. A scaffold write that fails removes the scaffold files this call
/// created; a price table that fails after the scaffold leaves the scaffold in place.
pub fn init(project: &Path, prices: &Path, options: InitOptions) -> Result<InitReport, InitError> {
    let kind = options.kind();
    let targets: Vec<PathBuf> = SCAFFOLD
        .iter()
        .map(|(name, _)| project.join(name))
        .collect();

    let mut present = Vec::new();
    for ((_, what), path) in SCAFFOLD.iter().zip(&targets) {
        if exists(path, what)? {
            present.push(SafeText::inline(&path.to_string_lossy()));
        }
    }
    if !present.is_empty() {
        return Err(InitError::Exists { paths: present });
    }

    let mut written: Vec<PathBuf> = Vec::new();
    for (index, ((_, what), path)) in SCAFFOLD.iter().zip(&targets).enumerate() {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        if let Err(error) = write_new(path, &options, scaffold_contents(index, kind), what) {
            for created in &written {
                // Best effort: the error being returned is the one that matters, and a file that cannot be removed
                // is left for the user, who sees the refusal.
                let _removed = fs::remove_file(created);
            }
            return Err(error);
        }
        written.push(path.clone());
    }

    let prices = if exists(prices, "the price table")? {
        PricesFile::Kept(prices.to_path_buf())
    } else {
        if let Some(parent) = prices.parent() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)
                .map_err(|source| InitError::Write {
                    what: "the price table's directory",
                    source,
                })?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        write_new(prices, &options, PRICES_TEMPLATE, "the price table")?;
        PricesFile::Written(prices.to_path_buf())
    };

    Ok(InitReport { written, prices })
}

/// Whether anything (a file, a directory, or a symlink, dangling or not) is at `path`.
fn exists(path: &Path, what: &'static str) -> Result<bool, InitError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(InitError::Write { what, source }),
    }
}

/// Creates `path` with `options` (which carry `create_new`), writes `contents`, and syncs it. A file this call
/// created but could not fill is removed.
fn write_new(
    path: &Path,
    options: &OpenOptions,
    contents: &str,
    what: &'static str,
) -> Result<(), InitError> {
    let mut file = options
        .open(path)
        .map_err(|source| InitError::Write { what, source })?;
    let filled = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all());
    if let Err(source) = filled {
        drop(file);
        let _removed = fs::remove_file(path);
        return Err(InitError::Write { what, source });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{InitKind, SCAFFOLD};

    #[test]
    fn kind_names_round_trip() -> Result<(), super::UnknownKind> {
        for kind in [InitKind::Tune, InitKind::Optimize, InitKind::Explore] {
            assert_eq!(kind.name().parse::<InitKind>()?, kind);
            assert_eq!(kind.to_string(), kind.name());
        }
        assert_eq!(SCAFFOLD.len(), 4);
        Ok(())
    }
}
