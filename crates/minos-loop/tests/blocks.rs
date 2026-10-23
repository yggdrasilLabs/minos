//! The loop's building blocks, each tested on `MemoryStore` without a VM (LOOP-09): the run inputs and their start
//! and resume checks (07-CONTEXT D-10, D-13, D-19) and the recorder that classifies append errors and dedupes
//! spends (D-08; 07-RESEARCH Pitfall 1).
//!
//! Fixtures are built from TOML text and fixed bytes through public constructors only. Candidate experiment IDs
//! come only from `Experiment::candidate` over a minted `CumulativeDiff`, because the loop's clippy policy bans
//! hashing a bare diff into an `ExperimentId` (D-12).

use std::cell::Cell;
use std::error::Error;
use std::str::FromStr;

use minos_core::diff::apply::Tree;
use minos_core::diff::cumulative::BaseTree;
use minos_core::id::{
    BlobId, ExperimentId, ImageDigest, ManifestId, ModelId, ReportHash, RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, Decision, DecisionKind, EXPECTED_KRUN_LAYERS, Event, Isolation,
    Origin, Record, ReviewAnswer, ReviewId, ReviewKind, SpendSource, StopReason,
};
use minos_core::manifest::{Arch, JudgeVersion, Manifest, Tier};
use minos_core::money::Prices;
use minos_core::outcome::{Outcome, Reason};
use minos_core::path::RelPath;
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;
use minos_loop::error::DriverError;
use minos_loop::inputs::{Resume, RunInputs, check_resume, check_start, same_frame};
use minos_loop::record::{append, event_kind, spend_once};
use minos_store::{Appended, Ledger, LedgerRead, MemoryStore, StoreError};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// A valid manifest: two writable files, one readable file, the judge under an excluded directory, and one
/// single-shot proposer naming model `haiku`.
const MANIFEST: &str = r#"[project]
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
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."haiku"]
input_token = "0.000001"
output_token = "0.000005"
cache_creation_input_token = "0.00000125"
cache_read_input_token = "0.0000001"
"#;

const PARSE_BASE: &str = "pub fn parse(input: &str) -> usize {\n    input.len()\n}\n";
const FMT_BASE: &str = "pub fn fmt(n: usize) -> String {\n    n.to_string()\n}\n";
const LIB: &str = "pub mod fmt;\npub mod parse;\n";
const PROGRAM: &[u8] = b"Make parse faster.\n";

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

/// The base surface.
fn base_tree() -> Result<Tree, Box<dyn Error>> {
    let mut tree = Tree::new();
    tree.insert(path("src/fmt.rs")?, FMT_BASE.as_bytes().to_vec());
    tree.insert(path("src/lib.rs")?, LIB.as_bytes().to_vec());
    tree.insert(path("src/parse.rs")?, PARSE_BASE.as_bytes().to_vec());
    Ok(tree)
}

/// The base handle under a fixed tree ID.
fn base() -> Result<BaseTree, Box<dyn Error>> {
    Ok(BaseTree::new(TreeId::of(b"base tarball"), base_tree()?))
}

/// The root image `minos.lock` names.
fn root() -> ImageDigest {
    ImageDigest::of(b"root image")
}

fn haiku() -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str("haiku")?)
}

/// Another judge version, under which no fixture's judge spec hashes.
fn other_judge() -> Result<JudgeVersion, Box<dyn Error>> {
    Ok(JudgeVersion::from_str(&"cd".repeat(32))?)
}

fn backend() -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        Isolation::Hardware,
        EXPECTED_KRUN_LAYERS.iter().copied().collect(),
        root(),
        Some(ImageDigest::of(b"kernel")),
    )
}

/// Everything a [`RunInputs`] borrows, owned.
struct Fixture {
    text: String,
    manifest: Manifest,
    base: BaseTree,
    prices: Prices,
}

impl Fixture {
    /// The fixture for `text`, priced by `prices`.
    fn new(text: &str, prices: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            text: text.to_owned(),
            manifest: Manifest::parse_toml(text)?,
            base: base()?,
            prices: Prices::parse_toml(prices)?,
        })
    }

    /// The standard manifest and prices.
    fn standard() -> Result<Self, Box<dyn Error>> {
        Self::new(MANIFEST, PRICES)
    }

    /// The judge version the fixture's lock names.
    fn judge(&self) -> JudgeVersion {
        JudgeVersion::of(&TreeId::of(b"judge data"), self.manifest.judge())
    }

    fn inputs(&self) -> RunInputs<'_> {
        RunInputs {
            manifest: &self.manifest,
            manifest_bytes: self.text.as_bytes(),
            base: &self.base,
            program: PROGRAM,
            prices: &self.prices,
            root_image: root(),
            judge: self.judge(),
        }
    }

    fn manifest_id(&self) -> ManifestId {
        ManifestId::of(self.text.as_bytes())
    }

    fn run(&self) -> RunId {
        RunId::of(&self.manifest_id(), &self.base.id(), STARTED_AT)
    }

    fn baseline(&self) -> ExperimentId {
        ExperimentId::baseline(&self.base.id())
    }

    /// `RunStarted` for this fixture's manifest and base, recording `root_image`.
    fn run_started(&self, root_image: Option<ImageDigest>) -> Event {
        Event::RunStarted {
            run: self.run(),
            manifest: self.manifest_id(),
            base: self.base.id(),
            baseline: self.baseline(),
            started_at: STARTED_AT,
            root_image,
        }
    }

    /// A scored-or-not trial of the baseline under `judge`.
    fn baseline_judged(&self, judge: JudgeVersion, trial: u32) -> Event {
        Event::Judged {
            exp: self.baseline(),
            judge,
            trial,
            outcome: Outcome::Rejected(Reason::BuildFailed),
            backend: backend(),
            logs: None,
        }
    }
}

fn state_of(events: Vec<Event>) -> Result<RunState, Box<dyn Error>> {
    let records: Vec<Record> = events.into_iter().map(Record::v1).collect();
    Ok(fold(&records)?)
}

/// A ledger that counts append attempts and forwards everything to a [`MemoryStore`].
struct Counting {
    inner: MemoryStore,
    appends: Cell<u32>,
}

impl Counting {
    fn new() -> Self {
        Self {
            inner: MemoryStore::new(),
            appends: Cell::new(0),
        }
    }
}

impl Ledger for Counting {
    fn append(&self, run: &RunId, record: Record) -> Result<Appended, StoreError> {
        self.appends.set(self.appends.get().saturating_add(1));
        self.inner.append(run, record)
    }

    fn read(&self, run: &RunId) -> Result<LedgerRead, StoreError> {
        self.inner.read(run)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// check_start
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn check_start_refuses_prices_without_the_proposers_model() -> TestResult {
    // Arrange
    let fixture = Fixture::new(MANIFEST, &PRICES.replace("\"haiku\"", "\"mid\""))?;

    // Act
    let result = check_start(&fixture.inputs());

    // Assert
    match result {
        Err(DriverError::PricesMissing { models }) => {
            assert_eq!(models, [haiku()?].into());
            Ok(())
        }
        other => Err(format!("expected PricesMissing, got {other:?}").into()),
    }
}

#[test]
fn check_start_accepts_complete_prices() -> TestResult {
    let fixture = Fixture::standard()?;
    check_start(&fixture.inputs())?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// check_resume: base, root image, judge
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn check_resume_refuses_another_base_naming_both() -> TestResult {
    // Arrange
    let fixture = Fixture::standard()?;
    let state = state_of(vec![fixture.run_started(Some(root()))])?;
    let other = BaseTree::new(TreeId::of(b"another tarball"), base_tree()?);
    let inputs = RunInputs {
        base: &other,
        ..fixture.inputs()
    };

    // Act
    let result = check_resume(&state, Some(&fixture.manifest), &inputs);

    // Assert
    match result {
        Err(DriverError::BaseMismatch { recorded, supplied }) => {
            assert_eq!(recorded, fixture.base.id());
            assert_eq!(supplied, other.id());
            Ok(())
        }
        other => Err(format!("expected BaseMismatch, got {other:?}").into()),
    }
}

#[test]
fn check_resume_refuses_a_run_that_recorded_no_root_image() -> TestResult {
    let fixture = Fixture::standard()?;
    let state = state_of(vec![fixture.run_started(None)])?;

    let result = check_resume(&state, Some(&fixture.manifest), &fixture.inputs());

    match result {
        Err(DriverError::RootImageChanged { recorded, current }) => {
            assert_eq!(recorded, None);
            assert_eq!(current, root());
            Ok(())
        }
        other => Err(format!("expected RootImageChanged, got {other:?}").into()),
    }
}

#[test]
fn check_resume_refuses_another_root_image_naming_both() -> TestResult {
    let fixture = Fixture::standard()?;
    let recorded_image = ImageDigest::of(b"an older root image");
    let state = state_of(vec![fixture.run_started(Some(recorded_image))])?;

    let result = check_resume(&state, Some(&fixture.manifest), &fixture.inputs());

    match result {
        Err(DriverError::RootImageChanged { recorded, current }) => {
            assert_eq!(recorded, Some(recorded_image));
            assert_eq!(current, root());
            Ok(())
        }
        other => Err(format!("expected RootImageChanged, got {other:?}").into()),
    }
}

#[test]
fn check_resume_refuses_a_baseline_judged_only_under_another_version() -> TestResult {
    // Arrange
    let fixture = Fixture::standard()?;
    let state = state_of(vec![
        fixture.run_started(Some(root())),
        fixture.baseline_judged(other_judge()?, 0),
        fixture.baseline_judged(other_judge()?, 1),
    ])?;

    // Act
    let result = check_resume(&state, Some(&fixture.manifest), &fixture.inputs());

    // Assert
    match result {
        Err(DriverError::JudgeChanged { recorded, current }) => {
            assert_eq!(recorded, other_judge()?);
            assert_eq!(current, fixture.judge());
            Ok(())
        }
        other => Err(format!("expected JudgeChanged, got {other:?}").into()),
    }
}

#[test]
fn check_resume_accepts_a_baseline_judged_under_the_current_version() -> TestResult {
    let fixture = Fixture::standard()?;
    let state = state_of(vec![
        fixture.run_started(Some(root())),
        fixture.baseline_judged(other_judge()?, 0),
        fixture.baseline_judged(fixture.judge(), 0),
    ])?;

    let resume = check_resume(&state, Some(&fixture.manifest), &fixture.inputs())?;

    assert_eq!(resume, Resume::Unchanged);
    Ok(())
}

#[test]
fn check_resume_accepts_a_run_with_no_judgment_yet() -> TestResult {
    let fixture = Fixture::standard()?;
    let state = state_of(vec![fixture.run_started(Some(root()))])?;

    let resume = check_resume(&state, Some(&fixture.manifest), &fixture.inputs())?;

    assert_eq!(resume, Resume::Unchanged);
    Ok(())
}

#[test]
fn check_resume_refuses_missing_prices_first() -> TestResult {
    let priced = Fixture::standard()?;
    let unpriced = Fixture::new(MANIFEST, &PRICES.replace("\"haiku\"", "\"mid\""))?;
    let state = state_of(vec![priced.run_started(None)])?;

    let result = check_resume(&state, Some(&priced.manifest), &unpriced.inputs());

    assert!(
        matches!(result, Err(DriverError::PricesMissing { .. })),
        "{result:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// check_resume: the manifest (D-19)
// ---------------------------------------------------------------------------------------------------------------

/// The recorded run: the standard manifest, one concluded generation with no valid candidate.
fn recorded_run() -> Result<(Fixture, RunState), Box<dyn Error>> {
    let fixture = Fixture::standard()?;
    let state = state_of(vec![
        fixture.run_started(Some(root())),
        fixture.baseline_judged(fixture.judge(), 0),
        Event::Selected {
            generation: 1,
            decision: Decision::new(DecisionKind::NoValidCandidates, 0, 2),
        },
    ])?;
    Ok((fixture, state))
}

/// The resume of [`recorded_run`] under the manifest `text`, with the recorded manifest supplied.
fn resume_under(text: &str) -> Result<Result<Resume, DriverError>, Box<dyn Error>> {
    let (recorded, state) = recorded_run()?;
    let current = Fixture::new(text, PRICES)?;
    assert_ne!(
        current.manifest_id(),
        recorded.manifest_id(),
        "the edit must change the manifest"
    );
    // The judge spec is in the frame, so a frame-preserving edit keeps the judge version.
    let inputs = RunInputs {
        judge: recorded.judge(),
        ..current.inputs()
    };
    Ok(check_resume(&state, Some(&recorded.manifest), &inputs))
}

#[test]
fn check_resume_with_the_recorded_manifest_is_unchanged() -> TestResult {
    let (fixture, state) = recorded_run()?;

    let resume = check_resume(&state, None, &fixture.inputs())?;

    assert_eq!(resume, Resume::Unchanged);
    Ok(())
}

#[test]
fn check_resume_continues_after_edits_outside_the_frame() -> TestResult {
    let param = "\n[[param]]\nname = \"a\"\nfile = \"src/parse.rs\"\nanchor = \"const A: i64 = \"\nmin = 0\nmax = 100\nstep = 5\n";
    let edits = [
        MANIFEST.replace("spend_usd = 40", "spend_usd = 80"),
        MANIFEST.replace("width = 4", "width = 4\nstall_step = 5"),
        MANIFEST.replace("batch = 4", "batch = 2"),
        format!("{MANIFEST}{param}"),
    ];
    for text in edits {
        // Act
        let resume = resume_under(&text)??;

        // Assert
        assert_eq!(resume, Resume::ManifestChanged { from_generation: 2 });
    }
    Ok(())
}

#[test]
fn check_resume_refuses_a_frame_edit_naming_both_ids() -> TestResult {
    let edits = [
        MANIFEST.replace(
            r#"writable = ["src/parse.rs", "src/fmt.rs"]"#,
            r#"writable = ["src/parse.rs"]"#,
        ),
        MANIFEST.replace(
            r#"run = ["./target/release/bench"]"#,
            r#"run = ["./target/release/bench", "--fast"]"#,
        ),
        MANIFEST.replace(
            r#"requires = { arch = "arm64" }"#,
            "requires = { arch = \"arm64\" }\nmemory_mib = 4096",
        ),
        MANIFEST.replace(
            r#"exclude = ["judge/"]"#,
            r#"exclude = ["judge/", "docs/"]"#,
        ),
    ];
    let recorded = Fixture::standard()?.manifest_id();
    for text in edits {
        // Act
        let result = resume_under(&text)?;

        // Assert
        match result {
            Err(DriverError::ManifestChanged {
                recorded: named,
                current,
            }) => {
                assert_eq!(named, recorded);
                assert_eq!(current, ManifestId::of(text.as_bytes()));
            }
            other => return Err(format!("expected ManifestChanged, got {other:?}").into()),
        }
    }
    Ok(())
}

#[test]
fn check_resume_refuses_a_changed_manifest_it_cannot_compare() -> TestResult {
    // Arrange
    let (recorded, state) = recorded_run()?;
    let current = Fixture::new(
        &MANIFEST.replace("spend_usd = 40", "spend_usd = 80"),
        PRICES,
    )?;
    let inputs = RunInputs {
        judge: recorded.judge(),
        ..current.inputs()
    };

    // Act
    let result = check_resume(&state, None, &inputs);

    // Assert
    assert!(
        matches!(result, Err(DriverError::ManifestChanged { .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn same_frame_holds_for_a_budget_edit_and_not_for_a_judge_edit() -> TestResult {
    let recorded = Manifest::parse_toml(MANIFEST)?;
    let budget = Manifest::parse_toml(&MANIFEST.replace("spend_usd = 40", "spend_usd = 80"))?;
    let judge = Manifest::parse_toml(
        &MANIFEST.replace(r#"min_improvement = "5%""#, r#"min_improvement = "10%""#),
    )?;

    assert!(same_frame(&recorded, &budget));
    assert!(!same_frame(&recorded, &judge));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The recorder
// ---------------------------------------------------------------------------------------------------------------

fn steered(note: &[u8]) -> Event {
    Event::Steered {
        generation: 1,
        ordinal: 0,
        note: BlobId::of(note),
    }
}

#[test]
fn append_of_a_new_and_an_equal_event_succeeds() -> TestResult {
    // Arrange
    let fixture = Fixture::standard()?;
    let store = MemoryStore::new();
    let run = fixture.run();
    append(&store, &run, fixture.run_started(Some(root())))?;

    // Act
    append(&store, &run, steered(b"note"))?;
    append(&store, &run, steered(b"note"))?;

    // Assert
    assert_eq!(store.read(&run)?.lines().len(), 2);
    Ok(())
}

#[test]
fn append_of_a_different_record_under_a_key_is_a_conflict() -> TestResult {
    // Arrange
    let fixture = Fixture::standard()?;
    let store = MemoryStore::new();
    let run = fixture.run();
    append(&store, &run, fixture.run_started(Some(root())))?;
    append(&store, &run, steered(b"first"))?;
    let before = store.read(&run)?.into_lines();

    // Act
    let result = append(&store, &run, steered(b"second"));

    // Assert
    match result {
        Err(DriverError::Conflict {
            run: named,
            seq,
            kind,
        }) => {
            assert_eq!(named, run);
            assert_eq!(seq, 1);
            assert_eq!(kind, "steered");
        }
        other => return Err(format!("expected Conflict, got {other:?}").into()),
    }
    assert_eq!(store.read(&run)?.into_lines(), before);
    Ok(())
}

#[test]
fn spend_once_appends_an_absent_spend_and_skips_a_recorded_one() -> TestResult {
    // Arrange
    let fixture = Fixture::standard()?;
    let store = Counting::new();
    let run = fixture.run();
    append(&store, &run, fixture.run_started(Some(root())))?;
    let source = SpendSource::Candidate {
        exp: fixture.baseline(),
        trial: 0,
    };
    let empty = fold(store.read(&run)?.records())?;

    // Act: the first spend is absent, so it is appended.
    spend_once(
        &store,
        &empty,
        &run,
        0,
        source.clone(),
        Usage::compute(1_000, 2_048),
    )?;
    let recorded = fold(store.read(&run)?.records())?;
    let attempts = store.appends.get();
    // A re-run trial measured a different usage; the first measurement stands.
    spend_once(
        &store,
        &recorded,
        &run,
        0,
        source,
        Usage::compute(9_000, 9_999),
    )?;

    // Assert
    assert_eq!(attempts, 2);
    assert_eq!(
        store.appends.get(),
        attempts,
        "a recorded spend attempts no append"
    );
    let lines = store.read(&run)?.into_lines();
    assert_eq!(lines.len(), 2);
    let spent = lines.last().map(|line| line.record().event().clone());
    assert!(
        matches!(spent, Some(Event::Spent { usage, .. }) if usage == Usage::compute(1_000, 2_048)),
        "{spent:?}"
    );
    Ok(())
}

#[test]
fn event_kind_names_every_variant_in_snake_case() -> TestResult {
    let fixture = Fixture::standard()?;
    let run = fixture.run();
    let request = ReviewId::of(&run, ReviewKind::Params, &BlobId::of(b"params"));
    let events = [
        (fixture.run_started(None), "run_started"),
        (
            Event::ManifestChanged {
                manifest: fixture.manifest_id(),
                from_generation: 2,
            },
            "manifest_changed",
        ),
        (
            Event::Proposed {
                exp: fixture.baseline(),
                parent: fixture.baseline(),
                generation: 1,
                tier: Tier::Sweep,
                diff: BlobId::of(b"diff"),
                rationale: BlobId::of(b"rationale"),
                origin: Origin::Sweep,
                batch: None,
            },
            "proposed",
        ),
        (fixture.baseline_judged(fixture.judge(), 0), "judged"),
        (
            Event::Selected {
                generation: 1,
                decision: Decision::new(DecisionKind::NoValidCandidates, 0, 0),
            },
            "selected",
        ),
        (
            Event::Spent {
                generation: 1,
                source: SpendSource::ModelCall {
                    call: 0,
                    model: haiku()?,
                },
                usage: Usage::compute(0, 0),
            },
            "spent",
        ),
        (steered(b"note"), "steered"),
        (
            Event::StopRequested {
                generation: 1,
                reason: StopReason::UserStop,
            },
            "stop_requested",
        ),
        (
            Event::ReviewRequested {
                request,
                kind: ReviewKind::Params,
            },
            "review_requested",
        ),
        (
            Event::Reviewed {
                request,
                answer: ReviewAnswer::Approved,
            },
            "reviewed",
        ),
        (
            Event::ReportAcknowledged {
                exp: fixture.baseline(),
                report: ReportHash::of(b"report"),
            },
            "report_acknowledged",
        ),
    ];
    for (event, kind) in events {
        assert_eq!(event_kind(&event), kind);
    }
    Ok(())
}
