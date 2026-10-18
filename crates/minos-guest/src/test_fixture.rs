//! Fixed jobs for the unit tests, built from the `minos.toml` example in CLAUDE.md through public constructors
//! only: build 2m, run 5m, the default judge budget, and the default memory (2048 MiB) and scratch (1024 MiB).

use std::error::Error;

use minos_core::diff::Diff;
use minos_core::experiment::Experiment;
use minos_core::guest::GuestJob;
use minos_core::id::{BlobId, TreeId};
use minos_core::job::{JobBudget, JobRequest};
use minos_core::limits::Limits;
use minos_core::manifest::{JudgeVersion, Manifest};

/// The `minos.toml` example in CLAUDE.md, section "Projects".
pub(crate) const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// A single-hunk modify of the manifest's one writable file.
const DIFF: &str = "\
diff --git a/src/parse.rs b/src/parse.rs
--- a/src/parse.rs
+++ b/src/parse.rs
@@ -1,3 +1,3 @@
 fn parse() {
-    old();
+    new();
 }
";

/// The base tarball bytes; the fixture's base tree is their hash.
pub(crate) const BASE: &[u8] = b"the base tarball bytes";

pub(crate) fn manifest() -> Result<Manifest, Box<dyn Error>> {
    Ok(Manifest::parse_toml(MANIFEST)?)
}

fn base() -> TreeId {
    TreeId::of(BASE)
}

fn judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::of(&TreeId::of(b"judge"), manifest()?.judge()))
}

pub(crate) fn diff() -> Result<Diff, Box<dyn Error>> {
    Ok(Diff::parse(
        DIFF,
        manifest()?.writable(),
        &Limits::standard(),
    )?)
}

fn budget() -> Result<JobBudget, Box<dyn Error>> {
    Ok(JobBudget::from_budget(manifest()?.budget()))
}

fn candidate_request() -> Result<JobRequest, Box<dyn Error>> {
    let diff = diff()?;
    let blob = BlobId::of(diff.as_bytes());
    let experiment = Experiment::candidate(base(), diff);
    Ok(JobRequest::candidate(
        &experiment,
        blob,
        judge()?,
        0,
        budget()?,
    )?)
}

/// A candidate job with a diff.
pub(crate) fn candidate_job() -> Result<GuestJob, Box<dyn Error>> {
    Ok(GuestJob::candidate(&manifest()?, &candidate_request()?))
}

/// The baseline candidate job: no diff.
pub(crate) fn baseline_job() -> Result<GuestJob, Box<dyn Error>> {
    let request = JobRequest::baseline(base(), judge()?, 0, budget()?)?;
    Ok(GuestJob::candidate(&manifest()?, &request))
}

/// The judge job for the candidate.
pub(crate) fn judge_job() -> Result<GuestJob, Box<dyn Error>> {
    Ok(GuestJob::judge(&manifest()?, &candidate_request()?))
}
