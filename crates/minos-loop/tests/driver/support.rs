//! Fakes and fixtures for the driver tests.
//!
//! - [`FakeJobs`] is the in-test [`JobRunner`]: it answers every trial from a closure, answers every preflight
//!   with a result chosen at construction, and records each request it ran. Its request list's lock, if
//!   poisoned, fails closed as an infrastructure failure, never a panic, and its `Debug` prints only a count.
//! - [`FaultStore`] forwards the ledger, blob, and lock traits to an inner store and returns one scripted
//!   [`StoreError`] on its Nth append, so a test can inject a conflict at an exact point.
//! - The fixtures are built from TOML text and fixed bytes through public constructors only. A candidate's
//!   experiment ID is always read from the ledger, never hashed by hand (07-CONTEXT D-12).
//! - A test that needs a scripted job runner (fail the first N calls, append a stop mid-launch, report another
//!   backend class) captures its own counter or store in the [`FakeJobs`] closure.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use minos_core::diff::apply::Tree;
use minos_core::diff::cumulative::BaseTree;
use minos_core::id::{BlobId, ImageDigest, MetricName, ModelId, RunId, TreeId};
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{
    BackendInfo, BackendKind, EXPECTED_KRUN_LAYERS, Event, Isolation, Record, StopReason,
};
use minos_core::manifest::{Arch, JudgeVersion, Manifest};
use minos_core::metric::{Metric, Scorecard};
use minos_core::money::Prices;
use minos_core::outcome::{Outcome, Reason};
use minos_core::path::RelPath;
use minos_core::usage::Usage;
use minos_loop::record::event_kind;
use minos_loop::{DriverOptions, JobFailure, JobRunner, LoopEvent, Observer, RunInputs};
use minos_propose::{FakeProvider, FakeReply, Proposer};
use minos_store::{Appended, Blobs, Ledger, LedgerRead, RunLock, StoreError};
use serde_json::{Map, Value};

/// A test's result.
pub type TestResult = Result<(), Box<dyn Error>>;

/// A fallible fixture.
pub type Fallible<T> = Result<T, Box<dyn Error>>;

/// The run's start time in Unix seconds.
pub const STARTED_AT: u64 = 1_760_000_000;

/// A sweep-only manifest: one integer param over a wide range, anchored in `src/parse.rs`, and a sweep proposer
/// with a batch of three. The default `[judge] trials` is 3.
pub const SWEEP_MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs", "src/fmt.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"
width = 4

[[param]]
name = "a"
file = "src/parse.rs"
anchor = "const A: i64 = "
min = 0
max = 1000
step = 1

[[proposer]]
kind = "sweep"
batch = 3
"#;

/// The same project with one single-shot proposer naming model `haiku`.
pub const SINGLE_SHOT_MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs", "src/fmt.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge/score"]

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
model = "haiku"
batch = 4
"#;

/// Rates for model `haiku` and the compute rates.
pub const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

/// Rates under which one fake trial costs exactly $1.50: a vCPU-second is $1 and memory is free, so the
/// candidate VM's 1,000 vCPU-ms cost $1.00 and the judge VM's 500 vCPU-ms cost $0.50. `haiku` keeps its rates.
pub const PRICES_DEAR: &str = r#"v = 1

[compute]
vcpu_second = "1"
gib_second = "0"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

/// `manifest` with `lines` added to its `[budget]` table, right after `spend_usd = 40`, and `spend_usd`
/// replaced by `spend_usd` dollars.
pub fn with_budget(manifest: &str, spend_usd: u64, lines: &str) -> String {
    manifest.replace(
        "spend_usd = 40\n",
        &format!("spend_usd = {spend_usd}\n{lines}"),
    )
}

/// Rates for another model only, so `haiku` is unpriced.
pub const PRICES_WITHOUT_HAIKU: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."sonnet"]
input_token = "0.000003"
output_token = "0.000015"
cache_creation_input_token = "0.00000375"
cache_read_input_token = "0.0000003"
"#;

/// The base `src/parse.rs`, holding the sweep's anchor line.
pub const PARSE_BASE: &str =
    "const A: i64 = 50;\npub fn parse(input: &str) -> usize {\n    input.len()\n}\n";
/// A model's rewrite of `src/parse.rs`.
pub const PARSE_FAST: &str =
    "const A: i64 = 50;\npub fn parse(input: &str) -> usize {\n    input.as_bytes().len()\n}\n";
/// Another rewrite of `src/parse.rs`.
pub const PARSE_FASTER: &str =
    "const A: i64 = 64;\npub fn parse(input: &str) -> usize {\n    input.as_bytes().len()\n}\n";
/// The base `src/fmt.rs`.
pub const FMT_BASE: &str = "pub fn fmt(n: usize) -> String {\n    n.to_string()\n}\n";
/// A model's rewrite of `src/fmt.rs`.
pub const FMT_FAST: &str = "pub fn fmt(n: usize) -> String {\n    format!(\"{n}\")\n}\n";
/// The readable `src/lib.rs`.
pub const LIB: &str = "pub mod fmt;\npub mod parse;\n";
/// The program file.
pub const PROGRAM: &[u8] = b"Make parse faster.\n";

// ---------------------------------------------------------------------------------------------------------------
// The fake job runner
// ---------------------------------------------------------------------------------------------------------------

/// The failure a poisoned request list fails closed as.
const POISONED: JobFailure = JobFailure::Infra {
    what: "the fake job runner's request list was poisoned",
};

/// A [`JobRunner`] that answers trials from `run` and preflights with `check`, recording every trial request.
pub struct FakeJobs<F> {
    run: F,
    check: Result<(), JobFailure>,
    requests: Mutex<Vec<JobRequest>>,
}

impl<F> FakeJobs<F>
where
    F: Fn(&JobRequest) -> Result<JobResult, JobFailure>,
{
    /// A runner whose preflight always passes.
    pub const fn new(run: F) -> Self {
        Self::with_check(run, Ok(()))
    }

    /// A runner whose preflight always answers `check`.
    pub const fn with_check(run: F, check: Result<(), JobFailure>) -> Self {
        Self {
            run,
            check,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Every trial request run so far, in order.
    pub fn requests(&self) -> Fallible<Vec<JobRequest>> {
        Ok(self
            .requests
            .lock()
            .map_err(|_poisoned| "the fake job runner's request list was poisoned")?
            .clone())
    }
}

impl<F> fmt::Debug for FakeJobs<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeJobs")
            .field(
                "requests",
                &self.requests.lock().map(|requests| requests.len()).ok(),
            )
            .finish_non_exhaustive()
    }
}

impl<F> JobRunner for FakeJobs<F>
where
    F: Fn(&JobRequest) -> Result<JobResult, JobFailure>,
{
    fn run(&self, request: &JobRequest) -> Result<JobResult, JobFailure> {
        self.requests
            .lock()
            .map_err(|_poisoned| POISONED)?
            .push(request.clone());
        (self.run)(request)
    }

    fn check(&self, _request: &JobRequest) -> Result<(), JobFailure> {
        self.check
    }
}

/// The candidate VM's fixed host-measured usage.
pub const fn candidate_usage() -> Usage {
    Usage::compute(1_000, 2_048_000)
}

/// The judge VM's fixed host-measured usage.
pub const fn judge_usage() -> Usage {
    Usage::compute(500, 1_024_000)
}

/// The fixed krun provenance every fake trial reports: krun on arm64.
pub fn backend() -> BackendInfo {
    backend_on(Arch::Arm64)
}

/// The fixed krun provenance on `arch`, for a trial that reports another backend class.
pub fn backend_on(arch: Arch) -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        arch,
        Isolation::Hardware,
        EXPECTED_KRUN_LAYERS.iter().copied().collect(),
        root(),
        Some(ImageDigest::of(b"kernel")),
    )
}

/// A scored result for `request`: the gate passes and the objective is `throughput`; both VMs ran.
///
/// # Errors
///
/// [`JobFailure::Config`] when the fixture's scorecard or result is invalid, which no test expects.
pub fn scored(request: &JobRequest, throughput: f64) -> Result<JobResult, JobFailure> {
    scored_on(request, throughput, backend())
}

/// A scored result for `request`, as [`scored`], reported under the provenance `backend`.
///
/// # Errors
///
/// [`JobFailure::Config`] when the fixture's scorecard or result is invalid, which no test expects.
pub fn scored_on(
    request: &JobRequest,
    throughput: f64,
    backend: BackendInfo,
) -> Result<JobResult, JobFailure> {
    let invalid = JobFailure::Config {
        what: "the fake scorecard is invalid",
    };
    let name = |text: &str| MetricName::from_str(text).map_err(|_| invalid);
    let value = |number: f64| Metric::new(number).map_err(|_| invalid);
    let card = Scorecard::from_entries([
        (name("identical_output")?, value(1.0)?, false),
        (name("throughput_mb_s")?, value(throughput)?, false),
    ])
    .map_err(|_| invalid)?;
    finish(request, Outcome::Scored(card), Some(judge_usage()), backend)
}

/// A result for `request` whose run stage failed, so the judge never ran.
///
/// # Errors
///
/// [`JobFailure::Config`] when the fixture's result is invalid, which no test expects.
pub fn run_failed(request: &JobRequest) -> Result<JobResult, JobFailure> {
    finish(
        request,
        Outcome::Rejected(Reason::RunFailed),
        None,
        backend(),
    )
}

/// The result of `request` with `outcome`, the fixed usages, no logs, and the provenance `backend`.
fn finish(
    request: &JobRequest,
    outcome: Outcome,
    judge: Option<Usage>,
    backend: BackendInfo,
) -> Result<JobResult, JobFailure> {
    JobResult::new(
        request,
        outcome,
        candidate_usage(),
        judge,
        backend,
        BTreeMap::new(),
    )
    .map_err(|_| JobFailure::Config {
        what: "the fake result is invalid",
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The fault-injecting store
// ---------------------------------------------------------------------------------------------------------------

/// A store that forwards to `inner` and fails its `fail_at`-th append (from 0) with one scripted error.
pub struct FaultStore<S> {
    inner: S,
    fail_at: usize,
    fault: Mutex<Option<StoreError>>,
    appends: AtomicUsize,
}

impl<S> FaultStore<S> {
    /// A store over `inner` whose append number `fail_at`, counting from 0, returns `fault` instead.
    pub const fn new(inner: S, fail_at: usize, fault: StoreError) -> Self {
        Self {
            inner,
            fail_at,
            fault: Mutex::new(Some(fault)),
            appends: AtomicUsize::new(0),
        }
    }
}

impl<S: Ledger> Ledger for FaultStore<S> {
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError> {
        if self.appends.fetch_add(1, Ordering::SeqCst) == self.fail_at
            && let Some(fault) = self
                .fault
                .lock()
                .map_err(|_poisoned| StoreError::Poisoned)?
                .take()
        {
            return Err(fault);
        }
        self.inner.append(run, record)
    }

    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError> {
        self.inner.read(run)
    }
}

impl<S: Blobs> Blobs for FaultStore<S> {
    fn put(&self, bytes: &[u8]) -> Result<BlobId, StoreError> {
        self.inner.put(bytes)
    }

    fn put_reader<R: std::io::Read>(&self, reader: R, max: u64) -> Result<BlobId, StoreError> {
        self.inner.put_reader(reader, max)
    }

    fn get(&self, id: &BlobId, max: u64) -> Result<Vec<u8>, StoreError> {
        self.inner.get(id, max)
    }

    fn contains(&self, id: &BlobId) -> Result<bool, StoreError> {
        self.inner.contains(id)
    }
}

impl<S: RunLock> RunLock for FaultStore<S> {
    type Guard<'a>
        = S::Guard<'a>
    where
        Self: 'a;

    fn lock_run(&self, run: &RunId) -> Result<Self::Guard<'_>, StoreError> {
        self.inner.lock_run(run)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The root image `minos.lock` names.
pub fn root() -> ImageDigest {
    ImageDigest::of(b"root image")
}

/// The model the single-shot proposer names.
pub fn haiku() -> Fallible<ModelId> {
    Ok(ModelId::from_str("haiku")?)
}

/// The base surface: three files, `src/parse.rs` holding the sweep's anchor.
fn base_tree() -> Fallible<Tree> {
    let mut tree = Tree::new();
    for (name, content) in [
        ("src/fmt.rs", FMT_BASE),
        ("src/lib.rs", LIB),
        ("src/parse.rs", PARSE_BASE),
    ] {
        tree.insert(RelPath::try_from(name)?, content.as_bytes().to_vec());
    }
    Ok(tree)
}

/// Everything a [`RunInputs`] borrows, owned.
pub struct Fixture {
    text: String,
    manifest: Manifest,
    base: BaseTree,
    prices: Prices,
}

impl Fixture {
    /// The fixture for manifest `text`, priced by `prices`.
    pub fn new(text: &str, prices: &str) -> Fallible<Self> {
        Ok(Self {
            text: text.to_owned(),
            manifest: Manifest::parse_toml(text)?,
            base: BaseTree::new(TreeId::of(b"base tarball"), base_tree()?),
            prices: Prices::parse_toml(prices)?,
        })
    }

    /// The sweep project.
    pub fn sweep() -> Fallible<Self> {
        Self::new(SWEEP_MANIFEST, PRICES)
    }

    /// The single-shot project.
    pub fn single_shot() -> Fallible<Self> {
        Self::new(SINGLE_SHOT_MANIFEST, PRICES)
    }

    /// The judge version the lock names: the judge data tree with the manifest's judge spec.
    pub fn judge(&self) -> JudgeVersion {
        JudgeVersion::of(&TreeId::of(b"judge"), self.manifest.judge())
    }

    /// The session inputs under the fixed root image.
    pub fn inputs(&self) -> RunInputs<'_> {
        self.inputs_with_root(root())
    }

    /// The session inputs under `root_image`.
    pub fn inputs_with_root(&self, root_image: ImageDigest) -> RunInputs<'_> {
        RunInputs {
            manifest: &self.manifest,
            manifest_bytes: self.text.as_bytes(),
            base: &self.base,
            program: PROGRAM,
            prices: &self.prices,
            root_image,
            judge: self.judge(),
        }
    }

    /// The run `Driver::start` at [`STARTED_AT`] names.
    pub fn run(&self) -> RunId {
        let inputs = self.inputs();
        RunId::of(&inputs.manifest_id(), &self.base.id(), STARTED_AT)
    }
}

/// An observer that keeps every event.
#[derive(Debug, Default)]
pub struct Recorder {
    events: Vec<LoopEvent>,
}

impl Recorder {
    /// Every event received, in order.
    pub fn events(&self) -> &[LoopEvent] {
        &self.events
    }
}

impl Observer for Recorder {
    fn observe(&mut self, event: &LoopEvent) {
        self.events.push(event.clone());
    }
}

/// Every event of `run`'s ledger with its seq, in seq order.
pub fn ledger<L: Ledger>(store: &L, run: &RunId) -> Fallible<Vec<(u64, Event)>> {
    Ok(store
        .read(run)?
        .lines()
        .iter()
        .map(|line| (line.seq(), line.record().event().clone()))
        .collect())
}

/// Fixed token usage for a model call.
pub const fn token_usage() -> Usage {
    Usage::tokens(100, 200, 0, 0)
}

/// One proposal of a single-shot reply: a rationale and the full contents of each changed file.
pub fn proposal(rationale: &str, files: &[(&str, &str)]) -> Value {
    let files = files
        .iter()
        .map(|(path, content)| {
            let mut file = Map::new();
            file.insert("path".to_owned(), Value::from(*path));
            file.insert("content".to_owned(), Value::from(*content));
            Value::Object(file)
        })
        .collect();
    let mut item = Map::new();
    item.insert("rationale".to_owned(), Value::from(rationale));
    item.insert("files".to_owned(), Value::Array(files));
    Value::Object(item)
}

/// A completed single-shot reply carrying `items`, with [`token_usage`] resolved to `haiku`.
pub fn reply(items: Vec<Value>) -> Fallible<FakeReply> {
    Ok(reply_from(haiku()?, token_usage(), items))
}

/// A completed single-shot reply carrying `items`, with `usage` resolved to `model`.
pub fn reply_from(model: ModelId, usage: Usage, items: Vec<Value>) -> FakeReply {
    let mut output = Map::new();
    output.insert("proposals".to_owned(), Value::Array(items));
    FakeReply::Structured {
        output: Value::Object(output),
        usage,
        models: BTreeMap::from([(model, usage)]),
    }
}

/// Session options with no retry pause.
pub const fn options() -> DriverOptions {
    DriverOptions {
        retry_pause: Duration::ZERO,
    }
}

/// A proposer with no provider, for the sweep.
pub const fn sweep_proposer() -> Proposer<FakeProvider> {
    Proposer::without_provider()
}

/// A proposer over a fake that answers with `script`.
pub fn single_shot_proposer(script: Vec<FakeReply>) -> Proposer<FakeProvider> {
    Proposer::with_provider(FakeProvider::new(script))
}

/// How many model calls the proposer's fake received.
pub fn calls(proposer: &Proposer<FakeProvider>) -> Fallible<usize> {
    let provider = proposer.provider().ok_or("the proposer has no provider")?;
    Ok(provider.requests()?.len())
}

/// The event kinds of a ledger, in seq order.
pub fn kinds(events: &[(u64, Event)]) -> Vec<&'static str> {
    events.iter().map(|(_, event)| event_kind(event)).collect()
}

/// Every `StopRequested` in `events`, in seq order.
pub fn stops(events: &[(u64, Event)]) -> Vec<(u32, StopReason)> {
    events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::StopRequested { generation, reason } => Some((*generation, *reason)),
            _ => None,
        })
        .collect()
}
