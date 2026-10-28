//! The small core seams the Phase 8 CLI composes (plan 08-01): the D-08 advisory arithmetic in `limits`
//! (`batch_output_tokens`, `near_output_cap`), the one run-cost sum `policy::run_cost`, and
//! `RunState::proposals`.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ExperimentId, ManifestId, ModelId, PromptHash, RunId, TreeId};
use minos_core::ledger::{Event, Origin, Record, SpendSource};
use minos_core::limits::{
    BYTES_PER_TOKEN_CONSERVATIVE, CLI_DEFAULT_OUTPUT_TOKENS, batch_output_tokens, near_output_cap,
};
use minos_core::manifest::{Manifest, Tier};
use minos_core::money::{Cost, CostError, Prices};
use minos_core::policy::{SpendCheck, run_cost, spend_ok};
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// Rates for model `mid` and nonzero compute rates.
const PRICES: &str = r#"v = 1

[compute]
vcpu_second = "0.0000276944"
gib_second = "0.0000036667"

[models."mid"]
input_token = "0.000003"
output_token = "0.000015"
cache_creation_input_token = "0.00000375"
cache_read_input_token = "0.0000003"
"#;

/// A valid manifest whose one single-shot proposer names model `mid`.
const MANIFEST: &str = r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]

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
command = ["./judge"]

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

// ---------------------------------------------------------------------------------------------------------------
// D-08 advisory arithmetic
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn d08_constants_are_pinned() {
    assert_eq!(CLI_DEFAULT_OUTPUT_TOKENS, 32_000);
    assert_eq!(BYTES_PER_TOKEN_CONSERVATIVE, 2);
}

#[test]
fn batch_output_tokens_is_the_ceiling_of_bytes_over_two() {
    assert_eq!(batch_output_tokens(10, 1), 5);
    assert_eq!(batch_output_tokens(11, 1), 6);
    assert_eq!(batch_output_tokens(0, 4), 0);
    assert_eq!(batch_output_tokens(10, 4), 20);
}

#[test]
fn batch_output_tokens_saturates() {
    assert_eq!(batch_output_tokens(u64::MAX, 16), u64::MAX);
    assert_eq!(batch_output_tokens(u64::MAX, 1), u64::MAX / 2 + 1);
}

#[test]
fn near_output_cap_fires_at_eighty_percent() {
    assert!(!near_output_cap(0));
    assert!(!near_output_cap(25_599));
    assert!(near_output_cap(25_600));
    assert!(near_output_cap(32_000));
    assert!(near_output_cap(u64::MAX));
}

// ---------------------------------------------------------------------------------------------------------------
// Ledger fixtures
// ---------------------------------------------------------------------------------------------------------------

fn base() -> TreeId {
    TreeId::of(b"t")
}

fn manifest_id() -> ManifestId {
    ManifestId::of(b"m")
}

fn baseline() -> ExperimentId {
    ExperimentId::baseline(&base())
}

/// A candidate experiment ID from a distinct number; IDs order as their numbers do.
fn exp(n: u64) -> Result<ExperimentId, Box<dyn Error>> {
    Ok(ExperimentId::from_str(&format!("{n:064x}"))?)
}

fn rec(event: Event) -> Record {
    Record::v1(event)
}

fn run_started() -> Record {
    rec(Event::RunStarted {
        run: RunId::of(&manifest_id(), &base(), STARTED_AT),
        manifest: manifest_id(),
        base: base(),
        baseline: baseline(),
        started_at: STARTED_AT,
        root_image: None,
    })
}

fn proposed(exp: ExperimentId, generation: u32) -> Result<Record, Box<dyn Error>> {
    Ok(rec(Event::Proposed {
        exp,
        parent: baseline(),
        generation,
        tier: Tier::SingleShot,
        diff: BlobId::of(exp.as_bytes()),
        rationale: BlobId::of(b"rationale"),
        origin: Origin::Model {
            model: ModelId::from_str("mid")?,
            prompt: PromptHash::of(b"prompt"),
        },
        batch: None,
    }))
}

fn spent(generation: u32, source: SpendSource, usage: Usage) -> Record {
    rec(Event::Spent {
        generation,
        source,
        usage,
    })
}

fn state(records: &[Record]) -> Result<RunState, Box<dyn Error>> {
    Ok(fold(records)?)
}

// ---------------------------------------------------------------------------------------------------------------
// run_cost
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn run_cost_sums_every_spend_as_spend_ok_does() -> TestResult {
    let prices = Prices::parse_toml(PRICES)?;
    let manifest = Manifest::parse_toml(MANIFEST)?;
    let mid = ModelId::from_str("mid")?;
    let candidate = Usage::compute(1_000, 2_048_000);
    let judge = Usage::compute(500, 1_024_000);
    let call = Usage::tokens(1_000, 100, 200, 300);
    let records = [
        run_started(),
        spent(
            0,
            SpendSource::Candidate {
                exp: baseline(),
                trial: 0,
            },
            candidate,
        ),
        spent(
            0,
            SpendSource::Judge {
                exp: baseline(),
                trial: 0,
            },
            judge,
        ),
        spent(
            1,
            SpendSource::ModelCall {
                call: 0,
                model: mid.clone(),
            },
            call,
        ),
    ];
    let state = state(&records)?;
    let expected = Cost::of(&prices, None, &candidate)?
        .checked_add(Cost::of(&prices, None, &judge)?)?
        .checked_add(Cost::of(&prices, Some(&mid), &call)?)?;
    let total = run_cost(&state, &prices)?;
    assert_eq!(total, expected);
    assert!(total.micro_dollars() > 0);
    match spend_ok(&state, &manifest, &prices)? {
        SpendCheck::Within { spent, .. } => assert_eq!(spent, total.approx()),
        SpendCheck::Stop(reason) => return Err(format!("stopped: {reason:?}").into()),
    }
    Ok(())
}

#[test]
fn run_cost_of_an_empty_run_is_zero() -> TestResult {
    let prices = Prices::parse_toml(PRICES)?;
    assert_eq!(run_cost(&state(&[run_started()])?, &prices)?, Cost::ZERO);
    Ok(())
}

#[test]
fn run_cost_of_an_unpriced_model_is_an_error() -> TestResult {
    let prices = Prices::parse_toml(PRICES)?;
    let opus = ModelId::from_str("opus")?;
    let records = [
        run_started(),
        spent(
            1,
            SpendSource::ModelCall {
                call: 0,
                model: opus.clone(),
            },
            Usage::tokens(0, 0, 0, 0),
        ),
    ];
    assert_eq!(
        run_cost(&state(&records)?, &prices),
        Err(CostError::UnpricedModel { model: opus })
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// RunState::proposals
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn proposals_lists_every_generation_in_experiment_id_order() -> TestResult {
    let records = [
        run_started(),
        proposed(exp(3)?, 1)?,
        proposed(exp(1)?, 2)?,
        proposed(exp(2)?, 1)?,
    ];
    let state = state(&records)?;
    let listed: Vec<(ExperimentId, u32)> = state
        .proposals()
        .map(|(id, proposal)| (*id, proposal.generation()))
        .collect();
    assert_eq!(listed, vec![(exp(1)?, 2), (exp(2)?, 1), (exp(3)?, 1)]);
    Ok(())
}

#[test]
fn proposals_is_empty_for_a_run_with_only_run_started() -> TestResult {
    assert_eq!(state(&[run_started()])?.proposals().count(), 0);
    Ok(())
}
