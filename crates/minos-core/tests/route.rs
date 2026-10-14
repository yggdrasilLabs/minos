//! Routing (CORE-11, D-13): eligibility with a reason for every ineligible tier, the route table, and the tier
//! for a generation, which climbs one rung per `stall_step` stalled generations, clamps at the top, and resets
//! after an improvement.
//!
//! Manifests are parsed from inline TOML and ledgers are folded from public constructors.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{BlobId, ExperimentId, ManifestId, RunId, TreeId};
use minos_core::ledger::{Decision, DecisionKind, Event, Origin, Record};
use minos_core::limits::SINGLE_SHOT_SURFACE_MAX_BYTES;
use minos_core::manifest::{Eligibility, IneligibleReason, Manifest, Tier};
use minos_core::route::{Route, RouteError, SurfaceStats, route, route_table};
use minos_core::state::{RunState, fold};

type TestResult = Result<(), Box<dyn Error>>;

/// The run's start time in Unix seconds.
const STARTED_AT: u64 = 1_760_000_000;

/// A sweep entry.
const SWEEP: &str = "[[proposer]]\nkind = \"sweep\"\nbatch = 4\n";
/// A single-shot entry with a model.
const SINGLE_SHOT: &str = "[[proposer]]\nkind = \"single_shot\"\nmodel = \"mid\"\nbatch = 4\n";
/// A single-shot entry without a model.
const SINGLE_SHOT_NO_MODEL: &str = "[[proposer]]\nkind = \"single_shot\"\nbatch = 4\n";
/// A planned entry, not in this release.
const PLANNED: &str = "[[proposer]]\nkind = \"planned\"\nmodel = \"mid\"\nbatch = 2\n";
/// An agentic entry, not in this release.
const AGENTIC: &str = "[[proposer]]\nkind = \"agentic\"\nmodel = \"mid\"\nbatch = 1\n";
/// A valid integer sweep param over the writable file.
const PARAM: &str = "[[param]]\nname = \"chunk\"\nfile = \"src/parse.rs\"\n\
                     anchor = \"const CHUNK: usize = \"\nmin = 1\nmax = 64\nstep = 1\n";

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// A valid manifest with these proposer entries, in this order, the stall step, and optionally the sweep param.
fn manifest(proposers: &[&str], stall_step: u32, param: bool) -> Result<Manifest, Box<dyn Error>> {
    let text = format!(
        r#"[project]
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
requires = {{ arch = "arm64" }}

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]

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
stall_step = {stall_step}

{}
{}"#,
        proposers.join("\n"),
        if param { PARAM } else { "" }
    );
    Ok(Manifest::parse_toml(&text)?)
}

/// The two-rung ladder of the behavior spec: sweep (with a param) and single-shot, stall step 2.
fn ladder() -> Result<Manifest, Box<dyn Error>> {
    manifest(&[SWEEP, SINGLE_SHOT], 2, true)
}

fn small() -> SurfaceStats {
    SurfaceStats::new(1_000)
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
    })
}

fn selected(generation: u32, kind: DecisionKind) -> Record {
    rec(Event::Selected {
        generation,
        decision: Decision::new(kind, 1, 0),
    })
}

fn sweep_proposal(exp: ExperimentId, generation: u32) -> Record {
    rec(Event::Proposed {
        exp,
        parent: baseline(),
        generation,
        tier: Tier::Sweep,
        diff: BlobId::of(exp.as_bytes()),
        rationale: BlobId::of(b"rationale"),
        origin: Origin::Sweep,
    })
}

/// A ledger whose generations 1 to `count` each ended with no valid candidate.
fn stalled(count: u32) -> Vec<Record> {
    let mut records = vec![run_started()];
    records.extend((1..=count).map(|generation| selected(generation, DecisionKind::NoValidCandidates)));
    records
}

fn state(records: &[Record]) -> Result<RunState, Box<dyn Error>> {
    Ok(fold(records)?)
}

fn routed(manifest: &Manifest, records: &[Record]) -> Result<Route, Box<dyn Error>> {
    Ok(route(manifest, &state(records)?, &small())?)
}

// ---------------------------------------------------------------------------------------------------------------
// Eligibility and the route table
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn single_shot_is_eligible_up_to_the_prompt_budget() -> TestResult {
    let manifest = ladder()?;
    let single = manifest
        .proposers()
        .iter()
        .find(|spec| spec.tier() == Tier::SingleShot)
        .ok_or("no single-shot entry")?;
    let at_limit = SurfaceStats::new(SINGLE_SHOT_SURFACE_MAX_BYTES);
    assert_eq!(at_limit.bytes(), SINGLE_SHOT_SURFACE_MAX_BYTES);
    assert_eq!(
        single.eligible(manifest.params(), &at_limit),
        Eligibility::Eligible
    );
    let over = SINGLE_SHOT_SURFACE_MAX_BYTES + 1;
    assert_eq!(
        single.eligible(manifest.params(), &SurfaceStats::new(over)),
        Eligibility::Ineligible(IneligibleReason::SurfaceTooLarge {
            bytes: over,
            limit: SINGLE_SHOT_SURFACE_MAX_BYTES,
        })
    );
    Ok(())
}

#[test]
fn surface_size_does_not_affect_a_sweep() -> TestResult {
    let manifest = ladder()?;
    let sweep = manifest
        .proposers()
        .iter()
        .find(|spec| spec.tier() == Tier::Sweep)
        .ok_or("no sweep entry")?;
    assert_eq!(
        sweep.eligible(manifest.params(), &SurfaceStats::new(u64::MAX)),
        Eligibility::Eligible
    );
    Ok(())
}

#[test]
fn a_declared_reason_comes_before_the_surface_check() -> TestResult {
    let manifest = manifest(&[SWEEP, SINGLE_SHOT_NO_MODEL], 2, true)?;
    let rows = route_table(&manifest, &SurfaceStats::new(u64::MAX));
    let single = rows
        .iter()
        .find(|row| row.tier() == Tier::SingleShot)
        .ok_or("no single-shot row")?;
    assert_eq!(
        single.eligibility(),
        Eligibility::Ineligible(IneligibleReason::NoModel)
    );
    Ok(())
}

#[test]
fn route_table_gives_a_reason_for_every_ineligible_tier() -> TestResult {
    let manifest = manifest(&[SWEEP, SINGLE_SHOT, PLANNED, AGENTIC], 2, false)?;
    let over = SINGLE_SHOT_SURFACE_MAX_BYTES + 7;
    let rows = route_table(&manifest, &SurfaceStats::new(over));
    let shown: Vec<(usize, Tier, Eligibility)> = rows
        .iter()
        .map(|row| (row.index(), row.tier(), row.eligibility()))
        .collect();
    assert_eq!(
        shown,
        vec![
            (
                0,
                Tier::Sweep,
                Eligibility::Ineligible(IneligibleReason::NoParams)
            ),
            (
                1,
                Tier::SingleShot,
                Eligibility::Ineligible(IneligibleReason::SurfaceTooLarge {
                    bytes: over,
                    limit: SINGLE_SHOT_SURFACE_MAX_BYTES,
                })
            ),
            (
                2,
                Tier::Planned,
                Eligibility::Ineligible(IneligibleReason::NotInThisRelease)
            ),
            (
                3,
                Tier::Agentic,
                Eligibility::Ineligible(IneligibleReason::NotInThisRelease)
            ),
        ]
    );
    Ok(())
}

#[test]
fn route_refuses_a_ladder_with_no_eligible_tier() -> TestResult {
    let manifest = manifest(&[SINGLE_SHOT], 2, false)?;
    let state = state(&stalled(0))?;
    let over = SurfaceStats::new(SINGLE_SHOT_SURFACE_MAX_BYTES + 1);
    assert!(matches!(
        route(&manifest, &state, &over),
        Err(RouteError::NoEligibleTier)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn route_climbs_one_rung_per_stall_step_and_clamps() -> TestResult {
    let manifest = ladder()?;
    let expected = [
        Tier::Sweep,
        Tier::Sweep,
        Tier::SingleShot,
        Tier::SingleShot,
        Tier::SingleShot,
        Tier::SingleShot,
        Tier::SingleShot,
    ];
    for (count, tier) in (0_u32..).zip(expected) {
        let route = routed(&manifest, &stalled(count))?;
        assert_eq!(route.tier(), tier, "after {count} stalled generations");
        assert_eq!(route.stall(), count);
        assert_eq!(route.step(), count / 2);
        let row = manifest
            .proposers()
            .get(route.proposer())
            .ok_or("the route names no proposer")?;
        assert_eq!(row.tier(), tier);
    }
    Ok(())
}

#[test]
fn kept_decisions_count_as_stalled() -> TestResult {
    let manifest = ladder()?;
    let records = vec![
        run_started(),
        selected(
            1,
            DecisionKind::Kept {
                incumbent: baseline(),
            },
        ),
        selected(2, DecisionKind::NoValidCandidates),
    ];
    assert_eq!(routed(&manifest, &records)?.tier(), Tier::SingleShot);
    Ok(())
}

#[test]
fn an_improvement_resets_routing_to_the_lowest_tier() -> TestResult {
    let manifest = ladder()?;
    let winner = exp(1)?;
    let mut records = stalled(3);
    assert_eq!(routed(&manifest, &records)?.tier(), Tier::SingleShot);
    records.extend([
        sweep_proposal(winner, 4),
        selected(4, DecisionKind::Improved { winner }),
    ]);
    let route = routed(&manifest, &records)?;
    assert_eq!((route.tier(), route.stall(), route.step()), (Tier::Sweep, 0, 0));
    records.push(selected(5, DecisionKind::NoValidCandidates));
    let route = routed(&manifest, &records)?;
    assert_eq!((route.tier(), route.stall()), (Tier::Sweep, 1));
    records.push(selected(6, DecisionKind::Kept { incumbent: winner }));
    let route = routed(&manifest, &records)?;
    assert_eq!((route.tier(), route.stall()), (Tier::SingleShot, 2));
    Ok(())
}

#[test]
fn the_ladder_is_ordered_by_tier_rank_whatever_the_declaration_order() -> TestResult {
    let manifest = manifest(&[SINGLE_SHOT, SWEEP], 1, true)?;
    let first = routed(&manifest, &stalled(0))?;
    assert_eq!(first.tier(), Tier::Sweep);
    let row = manifest
        .proposers()
        .get(first.proposer())
        .ok_or("the route names no proposer")?;
    assert_eq!(row.tier(), Tier::Sweep);
    assert_eq!(routed(&manifest, &stalled(1))?.tier(), Tier::SingleShot);
    Ok(())
}

#[test]
fn an_ineligible_rung_is_skipped() -> TestResult {
    // Sweep has no param, so the only rung is single-shot from the first generation.
    let manifest = manifest(&[SWEEP, SINGLE_SHOT], 1, false)?;
    let route = routed(&manifest, &stalled(0))?;
    assert_eq!(route.tier(), Tier::SingleShot);
    Ok(())
}
