//! The circuit breaker (CORE-17, D-14) and the spend check (D-09, COST-01).
//!
//! The breaker trips after exactly `max_empty_generations` consecutive generations with no valid candidate, or
//! exactly `max_infra_failures` consecutive infrastructure failures, and never one generation early. The spend
//! check prices every recorded usage from string rates and stops above the spend or token cap.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ExperimentId, ManifestId, ModelId, RunId, TreeId};
use minos_core::ledger::{Decision, DecisionKind, Event, Origin, Record, SpendSource, StopReason};
use minos_core::manifest::{Manifest, Tier};
use minos_core::money::{ApproxCost, CostError, Prices};
use minos_core::policy::{SpendCheck, circuit_breaker, spend_ok};
use minos_core::state::{RunState, fold};
use minos_core::usage::Usage;
use proptest::prelude::*;

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

/// Rates for model `mid` and free local compute.
const FREE_COMPUTE: &str = r#"v = 1

[compute]
vcpu_second = "0"
gib_second = "0"

[models."mid"]
input_token = "0.000003"
output_token = "0.000015"
cache_creation_input_token = "0.00000375"
cache_read_input_token = "0.0000003"
"#;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest with these `[budget]` lines after the stage budgets.
fn manifest(budget: &str) -> Result<Manifest, Box<dyn Error>> {
    let text = format!(
        r#"[project]
name = "logparse"
kind = "optimize"
program = "program.md"

[surface]
writable = ["src/parse.rs"]

[environment]
dockerfile = "minos.Dockerfile"
requires = {{ arch = "arm64" }}

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench"]

[budget]
build = "2m"
run = "5m"
{budget}

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
"#
    );
    Ok(Manifest::parse_toml(&text)?)
}

/// A manifest whose breaker trips after `empty` empty generations or `infra` infrastructure failures.
fn breaker_manifest(empty: u32, infra: u32) -> Result<Manifest, Box<dyn Error>> {
    manifest(&format!(
        "spend_usd = 40\nmax_empty_generations = {empty}\nmax_infra_failures = {infra}"
    ))
}

fn base() -> TreeId {
    TreeId::of(b"t")
}

fn manifest_id() -> ManifestId {
    ManifestId::of(b"m")
}

fn baseline() -> ExperimentId {
    ExperimentId::baseline(&base())
}

fn model(text: &str) -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str(text)?)
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
    })
}

fn selected(generation: u32, kind: DecisionKind) -> Record {
    rec(Event::Selected {
        generation,
        decision: Decision::new(kind, 1, 0),
    })
}

fn empty(generation: u32) -> Record {
    selected(generation, DecisionKind::NoValidCandidates)
}

fn kept(generation: u32) -> Record {
    selected(
        generation,
        DecisionKind::Kept {
            incumbent: baseline(),
        },
    )
}

fn model_call(
    generation: u32,
    call: u32,
    name: &str,
    usage: Usage,
) -> Result<Record, Box<dyn Error>> {
    Ok(rec(Event::Spent {
        generation,
        source: SpendSource::ModelCall {
            call,
            model: model(name)?,
        },
        usage,
    }))
}

fn candidate_run(generation: u32, trial: u32, usage: Usage) -> Record {
    rec(Event::Spent {
        generation,
        source: SpendSource::Candidate {
            exp: baseline(),
            trial,
        },
        usage,
    })
}

fn state(records: &[Record]) -> Result<RunState, Box<dyn Error>> {
    Ok(fold(records)?)
}

fn ledger(rest: impl IntoIterator<Item = Record>) -> Vec<Record> {
    std::iter::once(run_started()).chain(rest).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Circuit breaker
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn breaker_trips_at_exactly_max_empty_generations() -> TestResult {
    let manifest = breaker_manifest(3, 3)?;
    let budget = manifest.budget();
    assert_eq!(circuit_breaker(&state(&ledger([]))?, budget, 0), None);
    assert_eq!(
        circuit_breaker(&state(&ledger([empty(1), empty(2)]))?, budget, 0),
        None
    );
    assert_eq!(
        circuit_breaker(&state(&ledger([empty(1), empty(2), empty(3)]))?, budget, 0),
        Some(StopReason::NoValidCandidates)
    );
    Ok(())
}

#[test]
fn a_kept_generation_resets_the_empty_streak() -> TestResult {
    let manifest = breaker_manifest(3, 3)?;
    let budget = manifest.budget();
    let mut records = ledger([empty(1), kept(2), empty(3), empty(4)]);
    assert_eq!(circuit_breaker(&state(&records)?, budget, 0), None);
    records.push(empty(5));
    assert_eq!(
        circuit_breaker(&state(&records)?, budget, 0),
        Some(StopReason::NoValidCandidates)
    );
    Ok(())
}

#[test]
fn an_improvement_resets_the_empty_streak() -> TestResult {
    let manifest = breaker_manifest(2, 3)?;
    let winner = ExperimentId::from_str(&format!("{:064x}", 1))?;
    let records = ledger([
        empty(1),
        rec(Event::Proposed {
            exp: winner,
            parent: baseline(),
            generation: 2,
            tier: Tier::SingleShot,
            diff: BlobId::of(b"diff"),
            rationale: BlobId::of(b"rationale"),
            origin: Origin::Model {
                model: model("mid")?,
                prompt: minos_core::id::PromptHash::of(b"prompt"),
            },
        }),
        selected(2, DecisionKind::Improved { winner }),
        empty(3),
    ]);
    assert_eq!(
        circuit_breaker(&state(&records)?, manifest.budget(), 0),
        None
    );
    Ok(())
}

#[test]
fn a_missing_generation_ends_the_empty_streak() -> TestResult {
    let manifest = breaker_manifest(3, 3)?;
    let records = ledger([empty(1), empty(2), empty(4), empty(5)]);
    assert_eq!(
        circuit_breaker(&state(&records)?, manifest.budget(), 0),
        None
    );
    Ok(())
}

#[test]
fn breaker_trips_at_exactly_max_infra_failures() -> TestResult {
    let manifest = breaker_manifest(3, 3)?;
    let state = state(&ledger([]))?;
    assert_eq!(circuit_breaker(&state, manifest.budget(), 2), None);
    assert_eq!(
        circuit_breaker(&state, manifest.budget(), 3),
        Some(StopReason::InfraFailures)
    );
    assert_eq!(
        circuit_breaker(&state, manifest.budget(), u32::MAX),
        Some(StopReason::InfraFailures)
    );
    Ok(())
}

#[test]
fn an_empty_streak_takes_precedence_over_infra_failures() -> TestResult {
    let manifest = breaker_manifest(3, 3)?;
    let state = state(&ledger([empty(1), empty(2), empty(3)]))?;
    assert_eq!(
        circuit_breaker(&state, manifest.budget(), 3),
        Some(StopReason::NoValidCandidates)
    );
    Ok(())
}

proptest! {
    #[test]
    fn breaker_never_trips_early(
        (limit, streak) in (1_u32..=20).prop_flat_map(|limit| (Just(limit), 0..limit)),
        infra_limit in 1_u32..=20,
        lead in any::<bool>(),
    ) {
        let fail = |error: Box<dyn Error>| TestCaseError::fail(error.to_string());
        let manifest = breaker_manifest(limit, infra_limit).map_err(fail)?;
        // An optional kept generation before the streak, which must not count toward it.
        let offset = u32::from(lead);
        let mut records = vec![run_started()];
        if lead {
            records.push(kept(1));
        }
        records.extend((1..=streak).map(|n| empty(n + offset)));
        let infra = infra_limit - 1;
        let below = state(&records).map_err(fail)?;
        prop_assert_eq!(circuit_breaker(&below, manifest.budget(), infra), None);
        records.push(empty(streak + offset + 1));
        let at = state(&records).map_err(fail)?;
        let tripped = if streak + 1 == limit { Some(StopReason::NoValidCandidates) } else { None };
        prop_assert_eq!(circuit_breaker(&at, manifest.budget(), infra), tripped);
        prop_assert_eq!(
            circuit_breaker(&below, manifest.budget(), infra_limit),
            Some(StopReason::InfraFailures)
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Spend check
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn spend_within_the_caps_reports_the_cost_and_tokens() -> TestResult {
    let manifest = manifest("spend_usd = 40")?;
    let prices = Prices::parse_toml(PRICES)?;
    let records = ledger([
        model_call(1, 0, "mid", Usage::tokens(1_000, 100, 0, 0))?,
        model_call(1, 1, "mid", Usage::tokens(0, 0, 0, 0))?,
    ]);
    // 1000 input tokens at $0.000003 and 100 output tokens at $0.000015: $0.0045.
    assert_eq!(
        spend_ok(&state(&records)?, &manifest, &prices)?,
        SpendCheck::Within {
            spent: ApproxCost::from_micro(4_500),
            tokens: 1_100,
        }
    );
    Ok(())
}

#[test]
fn spend_above_the_cap_stops() -> TestResult {
    let manifest = manifest("spend_usd = 1")?;
    let prices = Prices::parse_toml(PRICES)?;
    // 333_334 input tokens at $0.000003: $1.000002, two micro-dollars over the cap.
    let over = ledger([model_call(1, 0, "mid", Usage::tokens(333_334, 0, 0, 0))?]);
    assert_eq!(
        spend_ok(&state(&over)?, &manifest, &prices)?,
        SpendCheck::Stop(StopReason::SpendCap)
    );
    // At one micro-dollar per token, a million tokens is exactly $1, which is not above the cap.
    let unit = Prices::parse_toml(
        &PRICES.replace(r#"input_token = "0.000003""#, r#"input_token = "0.000001""#),
    )?;
    let at = ledger([
        model_call(1, 0, "mid", Usage::tokens(400_000, 0, 0, 0))?,
        model_call(2, 0, "mid", Usage::tokens(600_000, 0, 0, 0))?,
    ]);
    assert_eq!(
        spend_ok(&state(&at)?, &manifest, &unit)?,
        SpendCheck::Within {
            spent: ApproxCost::from_micro(1_000_000),
            tokens: 1_000_000,
        }
    );
    let one_over = ledger([
        model_call(1, 0, "mid", Usage::tokens(400_000, 0, 0, 0))?,
        model_call(2, 0, "mid", Usage::tokens(600_001, 0, 0, 0))?,
    ]);
    assert_eq!(
        spend_ok(&state(&one_over)?, &manifest, &unit)?,
        SpendCheck::Stop(StopReason::SpendCap)
    );
    Ok(())
}

#[test]
fn tokens_above_the_cap_stop() -> TestResult {
    let manifest = manifest("spend_usd = 40\nmax_tokens = 1000")?;
    let prices = Prices::parse_toml(PRICES)?;
    let at = ledger([model_call(1, 0, "mid", Usage::tokens(600, 400, 0, 0))?]);
    assert!(matches!(
        spend_ok(&state(&at)?, &manifest, &prices)?,
        SpendCheck::Within { tokens: 1_000, .. }
    ));
    let over = ledger([
        model_call(1, 0, "mid", Usage::tokens(600, 400, 0, 0))?,
        model_call(1, 1, "mid", Usage::tokens(0, 0, 0, 1))?,
    ]);
    assert_eq!(
        spend_ok(&state(&over)?, &manifest, &prices)?,
        SpendCheck::Stop(StopReason::TokenCap)
    );
    Ok(())
}

#[test]
fn spend_of_an_unpriced_model_is_an_error() -> TestResult {
    let manifest = manifest("spend_usd = 40")?;
    let prices = Prices::parse_toml(PRICES)?;
    let records = ledger([model_call(1, 0, "opus", Usage::tokens(0, 0, 0, 0))?]);
    assert_eq!(
        spend_ok(&state(&records)?, &manifest, &prices),
        Err(CostError::UnpricedModel {
            model: model("opus")?
        })
    );
    Ok(())
}

#[test]
fn free_local_runs_stay_within_a_zero_cap() -> TestResult {
    let manifest = manifest("spend_usd = 0")?;
    let free = Prices::parse_toml(FREE_COMPUTE)?;
    let records = ledger([
        candidate_run(0, 0, Usage::compute(60_000, 4_096_000)),
        candidate_run(0, 1, Usage::compute(60_000, 4_096_000)),
    ]);
    assert_eq!(
        spend_ok(&state(&records)?, &manifest, &free)?,
        SpendCheck::Within {
            spent: ApproxCost::from_micro(0),
            tokens: 0,
        }
    );
    // The same runs priced at the published compute rates are above a zero cap.
    let priced = Prices::parse_toml(PRICES)?;
    assert_eq!(
        spend_ok(&state(&records)?, &manifest, &priced)?,
        SpendCheck::Stop(StopReason::SpendCap)
    );
    Ok(())
}
