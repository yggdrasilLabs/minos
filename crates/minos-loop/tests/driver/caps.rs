//! The metered caps at the driver's safe points (07-CONTEXT D-14, D-15; LOOP-07): a spend cap overshot by at
//! most one VM job, a token cap overshot by at most one model call, and a resolved model ID `prices.toml` does
//! not price, caught right after the first call that used it (D-13 extended; 07-RESEARCH Pitfall 7).
//!
//! Under [`PRICES_DEAR`] every fake trial costs exactly $1.50, so each cap's arithmetic is exact.

use std::str::FromStr;

use minos_core::id::ModelId;
use minos_core::ledger::{Event, SpendSource, StopReason};
use minos_core::usage::Usage;
use minos_loop::{Calibrated, Driver, DriverError, LoopEvent, NoopObserver, RunEnd};
use minos_store::MemoryStore;

use crate::support::{
    FakeJobs, Fixture, PARSE_FAST, PRICES, PRICES_DEAR, Recorder, SINGLE_SHOT_MANIFEST, STARTED_AT,
    SWEEP_MANIFEST, TestResult, calls, haiku, kinds, ledger, options, proposal, reply, reply_from,
    scored, single_shot_proposer, stops, sweep_proposer, with_budget,
};

/// A call's usage of 1,200 tokens, above the smallest token cap of 1,000.
const fn heavy_usage() -> Usage {
    Usage::tokens(600, 600, 0, 0)
}

/// Every model-call spend in `events`: its generation, call index, and model.
fn model_calls(events: &[(u64, Event)]) -> Vec<(u32, u32, ModelId)> {
    events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Spent {
                generation,
                source: SpendSource::ModelCall { call, model },
                ..
            } => Some((*generation, *call, model.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn spend_cap_below_one_job_records_the_first_baseline_trial_then_stops_with_spend_cap() -> TestResult
{
    // Arrange: a $1 cap, and a baseline trial that costs $1.50.
    let fixture = Fixture::new(&with_budget(SWEEP_MANIFEST, 1, ""), PRICES_DEAR)?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        Recorder::default(),
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let end = driver.run_to(2)?;

    // Assert: one job ran, its records landed, then the stop; nothing more was launched.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 0,
            reason: StopReason::SpendCap
        }
    );
    assert_eq!(jobs.requests()?.len(), 1);
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(
        kinds(&events),
        ["run_started", "spent", "spent", "judged", "stop_requested"]
    );
    assert_eq!(stops(&events), [(0, StopReason::SpendCap)]);
    let observed = driver.into_observer();
    assert!(matches!(
        observed.events(),
        [
            LoopEvent::Calibrated { trial: 0, .. },
            LoopEvent::Stopped {
                generation: 0,
                reason: StopReason::SpendCap
            }
        ]
    ));
    Ok(())
}

#[test]
fn calibrate_at_the_spend_cap_returns_calibrated_stopped() -> TestResult {
    // Arrange
    let fixture = Fixture::new(&with_budget(SWEEP_MANIFEST, 1, ""), PRICES_DEAR)?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let calibrated = driver.calibrate()?;

    // Assert
    assert_eq!(
        calibrated,
        Calibrated::Stopped {
            reason: StopReason::SpendCap
        }
    );
    assert_eq!(jobs.requests()?.len(), 1);
    Ok(())
}

#[test]
fn spend_cap_crossed_by_a_candidate_trial_overshoots_by_exactly_that_one_job() -> TestResult {
    // Arrange: a $5 cap; the three baseline trials cost $4.50, so the first candidate trial crosses it.
    let fixture = Fixture::new(&with_budget(SWEEP_MANIFEST, 5, ""), PRICES_DEAR)?;
    let store = MemoryStore::new();
    let proposer = sweep_proposer();
    let jobs =
        FakeJobs::new(|request| scored(request, if request.is_baseline() { 1.0 } else { 2.0 }));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let end = driver.run_to(2)?;

    // Assert: four jobs, the fourth recorded before the stop, and no further launch in the batch of three.
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::SpendCap
        }
    );
    let requests = jobs.requests()?;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests.iter().filter(|r| !r.is_baseline()).count(), 1);
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(
        kinds(&events).get(events.len().saturating_sub(4)..),
        Some(["spent", "spent", "judged", "stop_requested"].as_slice())
    );
    assert_eq!(stops(&events), [(1, StopReason::SpendCap)]);
    Ok(())
}

#[test]
fn token_cap_below_one_call_records_its_spend_and_proposals_then_stops_and_resumes_without_a_second_call()
-> TestResult {
    // Arrange: the smallest token cap, 1,000, and a first call that uses 1,200 tokens; a second reply waits.
    let capped = Fixture::new(
        &with_budget(SINGLE_SHOT_MANIFEST, 40, "max_tokens = 1000\n"),
        PRICES,
    )?;
    let store = MemoryStore::new();
    let item = proposal("bytes", &[("src/parse.rs", PARSE_FAST)]);
    let proposer = single_shot_proposer(vec![
        reply_from(haiku()?, heavy_usage(), vec![item.clone()]),
        reply(vec![item])?,
    ]);
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        capped.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let end = driver.run_to(3)?;

    // Assert: the call's spend, then its batch admitted and recorded, then the stop; no trial of generation 1
    // launched and no second call was paid for (WR-02).
    assert_eq!(
        end,
        RunEnd::Stopped {
            generation: 1,
            reason: StopReason::TokenCap
        }
    );
    assert_eq!(calls(&proposer)?, 1);
    let events = ledger(&store, &capped.run())?;
    assert_eq!(model_calls(&events), [(1, 0, haiku()?)]);
    assert_eq!(
        kinds(&events).get(events.len().saturating_sub(3)..),
        Some(["spent", "proposed", "stop_requested"].as_slice())
    );
    assert_eq!(stops(&events), [(1, StopReason::TokenCap)]);
    assert_eq!(jobs.requests()?.len(), 3);
    drop(driver);

    // Act: resume under a raised token cap, an edit outside the experiment frame.
    let raised = Fixture::new(
        &with_budget(SINGLE_SHOT_MANIFEST, 40, "max_tokens = 5000\n"),
        PRICES,
    )?;
    let mut resumed = Driver::resume(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        raised.inputs(),
        options(),
        capped.run(),
    )?;
    let end = resumed.run_to(1)?;

    // Assert: the recorded proposal's trial ran, the generation concluded, and the paid call was never repeated.
    assert_eq!(end, RunEnd::Reached { generation: 1 });
    assert_eq!(calls(&proposer)?, 1);
    assert_eq!(jobs.requests()?.len(), 4);
    let events = ledger(&store, &capped.run())?;
    assert_eq!(model_calls(&events), [(1, 0, haiku()?)]);
    assert_eq!(stops(&events), [(1, StopReason::TokenCap)]);
    Ok(())
}

#[test]
fn unpriced_resolved_model_stops_with_unpriced_model_after_one_call_spend() -> TestResult {
    // Arrange: `haiku` is priced, but the reply resolves the call to a model `prices.toml` does not name.
    let fixture = Fixture::single_shot()?;
    let resolved = ModelId::from_str("claude-haiku-4-5")?;
    let store = MemoryStore::new();
    let item = proposal("bytes", &[("src/parse.rs", PARSE_FAST)]);
    let proposer = single_shot_proposer(vec![
        reply_from(
            resolved.clone(),
            Usage::tokens(100, 200, 0, 0),
            vec![item.clone()],
        ),
        reply(vec![item])?,
    ]);
    let jobs = FakeJobs::new(|request| scored(request, 1.0));
    let mut driver = Driver::start(
        &store,
        &proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;

    // Act
    let result = driver.run_to(2);

    // Assert: the error names the resolved model; its one call's spend and proposals are recorded, and nothing
    // after them: no trial of generation 1 launched (WR-02).
    let Err(DriverError::UnpricedModel { model }) = result else {
        return Err(format!("expected UnpricedModel, got {result:?}").into());
    };
    assert_eq!(model, resolved);
    assert_eq!(calls(&proposer)?, 1);
    let events = ledger(&store, &fixture.run())?;
    assert_eq!(model_calls(&events), [(1, 0, resolved)]);
    assert_eq!(
        kinds(&events).get(events.len().saturating_sub(2)..),
        Some(["spent", "proposed"].as_slice())
    );
    assert!(stops(&events).is_empty());
    assert_eq!(jobs.requests()?.len(), 3);
    Ok(())
}
