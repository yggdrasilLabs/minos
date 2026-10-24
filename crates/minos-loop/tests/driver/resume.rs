//! Resume equivalence at every crash point (LOOP-05; 07-CONTEXT D-07, D-18; 07-RESEARCH Pitfalls 1 and 2).
//!
//! # Method
//!
//! A process that crashes just before its Nth append leaves exactly the ledger and blobs of the appends before
//! it. [`FaultStore`] produces that durable state without killing anything: it fails its Nth append with
//! [`StoreError::Poisoned`], which is not retryable, so the session ends with [`DriverError::Store`] and records
//! nothing more. A fresh [`Driver::resume`] then runs on the inner store with the same inputs to the same
//! absolute target, and the folded ledger is compared with the fold of an uninterrupted run.
//!
//! Every N from 1 (the first append after `RunStarted`, counting from 0) to the uninterrupted run's last append
//! is tried, so every boundary between two records is a crash point. The job runner's answer is a pure function
//! of the request ([`deterministic`]), and the sweep is seeded by run and generation, so the only freedom a
//! resumed session has is the order in which it records things, which the fold ignores.
//!
//! The tests also check that the crash points named in the plan were among those tried, by the records on
//! either side of each point in the uninterrupted ledger.
//!
//! # A re-run measures again (Pitfall 1)
//!
//! A real VM re-run after a crash measures a different usage. When the crash point falls after a trial's first
//! `Spent` and before its `Judged`, the resumed session's job runner reports a different candidate usage for its
//! first job, which is that re-run. The first measurement must stand: re-appending the trial's `Spent` with the
//! new usage would conflict, and the fold would differ.
//!
//! # The one allowed difference (D-07)
//!
//! A crash after a model call's `Spent` and before that generation's first `Proposed` leaves no record naming the
//! call's batch, so the resumed session proposes again under the next call index. Its proposals, judgments, and
//! decisions match the uninterrupted run, and its spends are the uninterrupted run's plus exactly that one
//! `Spent { ModelCall }`. At every other crash point the spends match exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use minos_core::id::ExperimentId;
use minos_core::job::{JobRequest, JobResult};
use minos_core::ledger::{Event, SpendId, SpendSource};
use minos_core::state::{Judgment, Proposal, RunState, Spend, fold};
use minos_core::usage::Usage;
use minos_loop::{Driver, DriverError, JobFailure, NoopObserver, RunEnd};
use minos_propose::{FakeProvider, Proposer};
use minos_store::{Ledger, MemoryStore, StoreError};

use crate::support::{
    FMT_FAST, FakeJobs, Fallible, FaultStore, Fixture, PARSE_FAST, STARTED_AT, TestResult,
    candidate_usage, deterministic, haiku, ledger, options, proposal, reply, single_shot_proposer,
    sweep_proposer, token_usage,
};

/// The absolute generation every run here is driven to.
const TARGET: u32 = 2;

/// The job runner every session here uses: answers depend on the request only.
type Jobs = FakeJobs<fn(&JobRequest) -> Result<JobResult, JobFailure>>;

/// A fresh [`Jobs`].
fn jobs() -> Jobs {
    FakeJobs::new(deterministic)
}

/// [`deterministic`]'s result for `request` with a candidate usage it never reports, as a re-run VM measures.
fn remeasured(request: &JobRequest) -> Result<JobResult, JobFailure> {
    let first = deterministic(request)?;
    let usage = candidate_usage();
    JobResult::new(
        request,
        first.outcome().clone(),
        Usage::compute(usage.vcpu_ms().saturating_add(1), usage.mem_mib_ms()),
        first.judge_usage(),
        first.backend().clone(),
        first.logs().clone(),
    )
    .map_err(|_| JobFailure::Config {
        what: "the re-measured result is invalid",
    })
}

/// Whether append `n` of the uninterrupted ledger `events` falls after a trial's first `Spent` and before its
/// `Judged`, so the resumed session's first job re-runs a trial with a spend already recorded.
fn rerun_after_spend(events: &[(u64, Event)], n: usize) -> bool {
    let before = n
        .checked_sub(1)
        .and_then(|m| events.get(m))
        .map(|(_, event)| event);
    matches!(
        (before, events.get(n).map(|(_, event)| event)),
        (
            Some(Event::Spent {
                source: SpendSource::Candidate { .. },
                ..
            }),
            Some(Event::Spent {
                source: SpendSource::Judge { .. },
                ..
            })
        ) | (Some(Event::Spent { .. }), Some(Event::Judged { .. }))
    )
}

/// An uninterrupted run: how many appends it made, its ledger, and its fold.
type Uninterrupted = (usize, Vec<(u64, Event)>, RunState);

/// The uninterrupted run of `fixture` to [`TARGET`]: how many appends it made, its ledger, and its fold.
fn uninterrupted(fixture: &Fixture, proposer: &Proposer<FakeProvider>) -> Fallible<Uninterrupted> {
    let store = FaultStore::new(MemoryStore::new(), usize::MAX, StoreError::Poisoned);
    let jobs = jobs();
    let mut driver = Driver::start(
        &store,
        proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )?;
    assert_eq!(
        driver.run_to(TARGET)?,
        RunEnd::Reached { generation: TARGET }
    );
    drop(driver);

    let run = fixture.run();
    let events = ledger(store.inner(), &run)?;
    // Every append landed as a new line, so append number N is line N of this ledger.
    assert_eq!(store.appends(), events.len());
    let read = store.inner().read(&run)?;
    Ok((store.appends(), events, fold(read.records())?))
}

/// A run of `fixture` that crashes before its append number `n` (from 0), then a fresh session resumed on what
/// survived and driven to [`TARGET`]; the resumed ledger's fold. `proposer` serves both sessions, as one
/// provider would across a restart. When `remeasure` holds, the resumed session's first job reports a new
/// candidate usage ([`remeasured`]).
fn crash_then_resume(
    fixture: &Fixture,
    proposer: &Proposer<FakeProvider>,
    n: usize,
    remeasure: bool,
) -> Fallible<RunState> {
    let store = FaultStore::new(MemoryStore::new(), n, StoreError::Poisoned);
    let jobs = jobs();
    let run = fixture.run();

    // The crashed session: it ends on the injected fault, and on nothing else.
    let mut crashed = Driver::start(
        &store,
        proposer,
        &jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        STARTED_AT,
    )
    .map_err(|error| format!("crash before append {n}: start failed: {error}"))?;
    match crashed.run_to(TARGET) {
        Err(DriverError::Store {
            source: StoreError::Poisoned,
            ..
        }) => {}
        Err(error) => {
            return Err(
                format!("crash before append {n}: the session failed otherwise: {error}").into(),
            );
        }
        Ok(end) => {
            return Err(
                format!("crash before append {n}: the session never reached it: {end:?}").into(),
            );
        }
    }
    drop(crashed);

    // The resumed session, on the inner store only: no fault remains to inject.
    let ran = AtomicUsize::new(0);
    let resumed_jobs = FakeJobs::new(|request: &JobRequest| {
        if remeasure && ran.fetch_add(1, Ordering::SeqCst) == 0 {
            remeasured(request)
        } else {
            deterministic(request)
        }
    });
    let mut resumed = Driver::resume(
        store.inner(),
        proposer,
        &resumed_jobs,
        NoopObserver,
        fixture.inputs(),
        options(),
        run,
    )
    .map_err(|error| format!("crash before append {n}: resume failed: {error}"))?;
    match resumed.run_to(TARGET) {
        Ok(RunEnd::Reached { generation: TARGET }) => {}
        Ok(end) => {
            return Err(
                format!("crash before append {n}: the resumed run ended at {end:?}").into(),
            );
        }
        // A `DriverError::Conflict` here would mean the resumed session re-recorded an effect differently.
        Err(error) => {
            return Err(format!("crash before append {n}: the resumed run failed: {error}").into());
        }
    }
    drop(resumed);

    let read = store.inner().read(&run)?;
    Ok(fold(read.records())?)
}

/// A crash between a baseline trial's last `Spent` and its `Judged`.
const BASELINE_SPENT_JUDGED: &str = "between a baseline Spent and its Judged";
/// A crash after the batch blob is stored and before the batch's first `Proposed`.
const BLOB_FIRST_PROPOSED: &str = "between a batch blob put and the batch's first Proposed";
/// A crash in the middle of a batch's `Proposed` records.
const PROPOSED_PROPOSED: &str = "between two Proposed of one batch";
/// A crash between a candidate trial's two spends.
const CANDIDATE_JUDGE_SPENT: &str =
    "between a candidate's Spent { Candidate } and its Spent { Judge }";
/// A crash after a generation's last trial and before its conclusion.
const JUDGED_SELECTED: &str = "between a generation's last Judged and its Selected";

/// The named crash points the plan requires.
const NAMED_POINTS: [&str; 5] = [
    BASELINE_SPENT_JUDGED,
    BLOB_FIRST_PROPOSED,
    PROPOSED_PROPOSED,
    CANDIDATE_JUDGE_SPENT,
    JUDGED_SELECTED,
];

/// Which named crash point, if any, lies between `before` and `after`.
fn named_point(before: &Event, after: &Event) -> Option<&'static str> {
    let name = match (before, after) {
        (
            Event::Spent {
                generation: 0,
                source: SpendSource::Judge { .. },
                ..
            },
            Event::Judged { .. },
        ) => BASELINE_SPENT_JUDGED,
        (Event::Proposed { .. }, Event::Proposed { .. }) => PROPOSED_PROPOSED,
        // The batch blob is stored right before the batch's first `Proposed`, which no `Proposed` precedes.
        (_, Event::Proposed { .. }) => BLOB_FIRST_PROPOSED,
        (
            Event::Spent {
                source: SpendSource::Candidate { .. },
                generation,
                ..
            },
            Event::Spent {
                source: SpendSource::Judge { .. },
                ..
            },
        ) if *generation > 0 => CANDIDATE_JUDGE_SPENT,
        (Event::Judged { .. }, Event::Selected { .. }) => JUDGED_SELECTED,
        _ => return None,
    };
    Some(name)
}

/// Every crash point from 1 to the last append, with the named point it is, if any. Fails unless every named
/// point is among them.
fn crash_points(
    appends: usize,
    events: &[(u64, Event)],
) -> Fallible<Vec<(usize, Option<&'static str>)>> {
    let points: Vec<(usize, Option<&'static str>)> = (1..appends)
        .map(|n| {
            let before = n.checked_sub(1).and_then(|m| events.get(m));
            let name = match (before, events.get(n)) {
                (Some((_, before)), Some((_, after))) => named_point(before, after),
                _ => None,
            };
            (n, name)
        })
        .collect();
    let covered: BTreeSet<&str> = points.iter().filter_map(|(_, name)| *name).collect();
    for name in NAMED_POINTS {
        if !covered.contains(name) {
            return Err(format!("no crash point lies {name}").into());
        }
    }
    Ok(points)
}

#[test]
fn sweep_resumed_after_a_crash_before_any_append_folds_to_the_uninterrupted_state() -> TestResult {
    // Arrange: the uninterrupted run and its crash points.
    let fixture = Fixture::sweep()?;
    let (appends, events, expected) = uninterrupted(&fixture, &sweep_proposer())?;
    let points = crash_points(appends, &events)?;

    for (n, name) in points {
        // Act
        let resumed = crash_then_resume(
            &fixture,
            &sweep_proposer(),
            n,
            rerun_after_spend(&events, n),
        )?;

        // Assert: the whole fold, partial batches and re-run trials included.
        if resumed != expected {
            return Err(format!(
                "crash before append {n} ({}): the resumed fold differs from the uninterrupted one",
                name.unwrap_or("unnamed point")
            )
            .into());
        }
    }
    Ok(())
}

/// The replies of the single-shot fake: the same reply every time, so a re-proposal after a crash answers
/// exactly what the lost call did. Generation 1 admits both rewrites; in generation 2 the winner's file is
/// already rewritten, so that proposal is dropped as no change and the other is admitted.
fn single_shot_script() -> Fallible<Proposer<FakeProvider>> {
    let mut script = Vec::new();
    for _ in 0..4 {
        script.push(reply(vec![
            proposal("faster parse", &[("src/parse.rs", PARSE_FAST)]),
            proposal("faster fmt", &[("src/fmt.rs", FMT_FAST)]),
        ])?);
    }
    Ok(single_shot_proposer(script))
}

/// What the model tier must reproduce exactly: every decision, every proposal, and every judgment.
type Projection = (
    Vec<(u32, minos_core::ledger::Decision)>,
    Vec<(ExperimentId, Proposal)>,
    Vec<(ExperimentId, u32, Judgment)>,
);

/// `state`'s [`Projection`] under `fixture`'s judge.
fn project(fixture: &Fixture, state: &RunState) -> Projection {
    let decisions = state
        .decisions()
        .map(|(generation, decision)| (generation, *decision))
        .collect();
    let proposals: Vec<(ExperimentId, Proposal)> = (1..=TARGET)
        .flat_map(|generation| state.proposals_in(generation))
        .map(|(exp, proposal)| (*exp, proposal.clone()))
        .collect();
    let judge = fixture.judge();
    let judgments = std::iter::once(*state.baseline())
        .chain(proposals.iter().map(|(exp, _)| *exp))
        .flat_map(|exp| {
            state
                .trials(&exp, &judge)
                .map(move |(trial, judgment)| (exp, trial, judgment.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    (decisions, proposals, judgments)
}

/// Every spend of `state`, by ID.
fn spends(state: &RunState) -> BTreeMap<SpendId, Spend> {
    state
        .spends()
        .map(|(id, spend)| (*id, spend.clone()))
        .collect()
}

#[test]
fn single_shot_resumed_after_any_crash_matches_but_for_one_repeated_call_spend() -> TestResult {
    // Arrange: the uninterrupted run and its crash points.
    let fixture = Fixture::single_shot()?;
    let (appends, events, expected) = uninterrupted(&fixture, &single_shot_script()?)?;
    let points = crash_points(appends, &events)?;
    let expected_projection = project(&fixture, &expected);
    let expected_spends = spends(&expected);
    let model = haiku()?;
    let run = fixture.run();
    let mut repeated_calls = 0_u32;

    for (n, name) in points {
        let at = format!(
            "crash before append {n} ({})",
            name.unwrap_or("unnamed point")
        );

        // Act
        let resumed = crash_then_resume(
            &fixture,
            &single_shot_script()?,
            n,
            rerun_after_spend(&events, n),
        )?;

        // Assert: proposals, judgments, and decisions exactly.
        if project(&fixture, &resumed) != expected_projection {
            return Err(
                format!("{at}: the resumed proposals, judgments, or decisions differ").into(),
            );
        }

        // Assert: every uninterrupted spend, unchanged.
        let mut extra = spends(&resumed);
        for (id, spend) in &expected_spends {
            if extra.remove(id).as_ref() != Some(spend) {
                return Err(format!("{at}: an uninterrupted spend is missing or changed").into());
            }
        }

        // Assert: one more model call spend exactly when the crash fell between a call's `Spent` and the
        // generation's first `Proposed`, keyed under the next call index.
        let repeated_call = match (n.checked_sub(1).and_then(|m| events.get(m)), events.get(n)) {
            (
                Some((
                    _,
                    Event::Spent {
                        generation,
                        source: SpendSource::ModelCall { call, .. },
                        ..
                    },
                )),
                Some((_, Event::Proposed { .. })),
            ) => Some((*generation, call.saturating_add(1))),
            _ => None,
        };
        match repeated_call {
            None if extra.is_empty() => {}
            None => {
                return Err(
                    format!("{at}: the resumed run has {} extra spends", extra.len()).into(),
                );
            }
            Some((generation, call)) => {
                repeated_calls = repeated_calls.saturating_add(1);
                let source = SpendSource::ModelCall {
                    call,
                    model: model.clone(),
                };
                let id = SpendId::of(&run, generation, &source);
                let only = extra.len() == 1
                    && extra.get(&id).is_some_and(|spend| {
                        spend.generation() == generation
                            && *spend.source() == source
                            && *spend.usage() == token_usage()
                    });
                if !only {
                    return Err(format!(
                        "{at}: expected one repeated model call spend of generation {generation}, found {}",
                        extra.len()
                    )
                    .into());
                }
            }
        }
    }

    // Every generation had one crash point between its call's spend and its first proposal.
    assert_eq!(repeated_calls, TARGET);
    Ok(())
}
