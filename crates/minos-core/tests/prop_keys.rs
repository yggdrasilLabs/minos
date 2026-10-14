//! The frozen ledger: one content-derived `EventKey` per event, the `Record` and `Line` wire shapes, and the
//! closed payload enums.
//!
//! Key distinctness is checked against an independent model: each test-side `Spec` names its key fields, and two
//! events share a key exactly when their key fields are equal. Round trips use `serde_json` directly; golden wire
//! files begin in plan 03-09, after the freeze.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::str::FromStr;

use minos_core::id::{
    BlobId, ExperimentId, IdParseError, ImageDigest, ManifestId, MetricName, ModelId, PromptHash,
    ReportHash, RunId, TreeId,
};
use minos_core::ledger::{
    BackendInfo, BackendKind, ConfinementLayer, Decision, DecisionKind, EXPECTED_KRUN_LAYERS,
    Event, EventKey, Isolation, Line, LineError, Origin, Record, ReviewAnswer, ReviewId,
    ReviewKind, SpendId, SpendSource, StopReason,
};
use minos_core::manifest::{Arch, JudgeVersion, Tier};
use minos_core::metric::{Metric, Scorecard};
use minos_core::outcome::{Outcome, Reason, Stage};
use minos_core::usage::Usage;
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

/// The start time of the `golden_ids.rs` `RunId`.
const STARTED_AT: u64 = 1_760_000_000;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------------------------

/// The `RunId` of `golden_ids.rs`: manifest `m`, base tree `t`, started at [`STARTED_AT`].
fn run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), STARTED_AT)
}

/// A second run of the same project one second later.
fn other_run() -> RunId {
    RunId::of(&ManifestId::of(b"m"), &TreeId::of(b"t"), STARTED_AT + 1)
}

fn exp(seed: u8) -> ExperimentId {
    ExperimentId::baseline(&TreeId::of(&[b'e', seed]))
}

fn judge(seed: u8) -> Result<JudgeVersion, IdParseError> {
    JudgeVersion::from_str(&format!("{seed:02x}").repeat(32))
}

fn haiku() -> Result<ModelId, Box<dyn Error>> {
    Ok(ModelId::from_str("haiku")?)
}

fn backend(isolation: Isolation, layers: &[ConfinementLayer]) -> BackendInfo {
    BackendInfo::new(
        BackendKind::Krun,
        Arch::Arm64,
        isolation,
        layers.iter().copied().collect(),
        ImageDigest::of(b"root"),
        Some(ImageDigest::of(b"kernel")),
    )
}

fn scored(value: f64) -> Result<Outcome, Box<dyn Error>> {
    let card = Scorecard::from_entries([(
        MetricName::from_str("throughput_mb_s")?,
        Metric::new(value)?,
        false,
    )])?;
    Ok(Outcome::Scored(card))
}

fn review(kind: ReviewKind, subject: u8) -> ReviewId {
    ReviewId::of(&run(), kind, &BlobId::of(&[b'r', subject]))
}

/// One event of each of the eleven kinds, in key-table order.
fn one_of_each() -> Result<Vec<Event>, Box<dyn Error>> {
    let base = TreeId::of(b"t");
    Ok(vec![
        Event::RunStarted {
            run: run(),
            manifest: ManifestId::of(b"m"),
            base,
            baseline: ExperimentId::baseline(&base),
            started_at: STARTED_AT,
        },
        Event::ManifestChanged {
            manifest: ManifestId::of(b"m2"),
            from_generation: 2,
        },
        Event::Proposed {
            exp: exp(1),
            parent: ExperimentId::baseline(&base),
            generation: 1,
            tier: Tier::SingleShot,
            diff: BlobId::of(b"diff"),
            rationale: BlobId::of(b"rationale"),
            origin: Origin::Model {
                model: haiku()?,
                prompt: PromptHash::of(b"prompt"),
            },
        },
        Event::Judged {
            exp: exp(1),
            judge: judge(1)?,
            trial: 0,
            outcome: scored(12.25)?,
            backend: backend(Isolation::Hardware, &EXPECTED_KRUN_LAYERS),
        },
        Event::Selected {
            generation: 1,
            decision: Decision::new(DecisionKind::Improved { winner: exp(1) }, 4, 1),
        },
        Event::Spent {
            generation: 1,
            source: SpendSource::ModelCall {
                call: 0,
                model: haiku()?,
            },
            usage: Usage::tokens(100, 20, 3, 4),
        },
        Event::Steered {
            generation: 2,
            ordinal: 0,
            note: BlobId::of(b"note"),
        },
        Event::StopRequested {
            generation: 2,
            reason: StopReason::UserStop,
        },
        Event::ReviewRequested {
            request: review(ReviewKind::Params, 0),
            kind: ReviewKind::Params,
        },
        Event::Reviewed {
            request: review(ReviewKind::Params, 0),
            answer: ReviewAnswer::Approved,
        },
        Event::ReportAcknowledged {
            exp: exp(1),
            report: ReportHash::of(b"report"),
        },
    ])
}

/// The `snake_case` variant name each event in [`one_of_each`] serializes under.
const VARIANT_NAMES: [&str; 11] = [
    "run_started",
    "manifest_changed",
    "proposed",
    "judged",
    "selected",
    "spent",
    "steered",
    "stop_requested",
    "review_requested",
    "reviewed",
    "report_acknowledged",
];

fn json(event: Event) -> Result<String, Box<dyn Error>> {
    Ok(serde_json::to_string(&Record::v1(event))?)
}

fn record_rejected(text: &str) -> Result<(), Box<dyn Error>> {
    match serde_json::from_str::<Record>(text) {
        Ok(_) => Err(format!("record accepted: {text}").into()),
        Err(_) => Ok(()),
    }
}

fn run_started_json() -> Result<String, Box<dyn Error>> {
    let event = one_of_each()?
        .into_iter()
        .next()
        .ok_or("one_of_each is empty")?;
    json(event)
}

// ---------------------------------------------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn key_table_covers_every_variant() -> TestResult {
    let run = run();
    let events = one_of_each()?;
    assert_eq!(events.len(), 11);
    let keys: BTreeSet<EventKey> = events.iter().map(|event| event.key(&run)).collect();
    assert_eq!(keys.len(), 11);
    Ok(())
}

#[test]
fn the_same_event_under_two_runs_gives_two_keys() -> TestResult {
    for event in one_of_each()? {
        assert_ne!(event.key(&run()), event.key(&other_run()));
    }
    Ok(())
}

#[test]
fn non_key_fields_do_not_change_key() -> TestResult {
    let run = run();
    let judged = |outcome: Outcome, backend: BackendInfo| -> Result<Event, IdParseError> {
        Ok(Event::Judged {
            exp: exp(1),
            judge: judge(1)?,
            trial: 2,
            outcome,
            backend,
        })
    };
    let a = judged(
        scored(1.0)?,
        backend(Isolation::Hardware, &EXPECTED_KRUN_LAYERS),
    )?;
    let b = judged(
        Outcome::Crashed { stage: Stage::Run },
        backend(Isolation::WeakProcess, &[ConfinementLayer::Seatbelt]),
    )?;
    let c = judged(
        Outcome::Rejected(Reason::BuildFailed),
        backend(Isolation::Hardware, &[]),
    )?;
    assert_ne!(a, b);
    assert_eq!(a.key(&run), b.key(&run));
    assert_eq!(a.key(&run), c.key(&run));

    let proposed =
        |rationale: &[u8], origin: Origin, tier: Tier, parent: ExperimentId| Event::Proposed {
            exp: exp(3),
            parent,
            generation: 1,
            tier,
            diff: BlobId::of(b"diff"),
            rationale: BlobId::of(rationale),
            origin,
        };
    let p1 = proposed(b"one", Origin::Sweep, Tier::Sweep, exp(0));
    let p2 = proposed(
        b"two",
        Origin::Model {
            model: haiku()?,
            prompt: PromptHash::of(b"p"),
        },
        Tier::SingleShot,
        exp(1),
    );
    assert_ne!(p1, p2);
    assert_eq!(p1.key(&run), p2.key(&run));

    let spent = |usage: Usage| -> Result<Event, Box<dyn Error>> {
        Ok(Event::Spent {
            generation: 1,
            source: SpendSource::ModelCall {
                call: 3,
                model: haiku()?,
            },
            usage,
        })
    };
    let s1 = spent(Usage::tokens(1, 2, 3, 4))?;
    let s2 = spent(Usage::compute(10, 20))?;
    assert_ne!(s1, s2);
    assert_eq!(s1.key(&run), s2.key(&run));

    let start = |manifest: &[u8], started_at: u64| Event::RunStarted {
        run,
        manifest: ManifestId::of(manifest),
        base: TreeId::of(b"t"),
        baseline: ExperimentId::baseline(&TreeId::of(b"t")),
        started_at,
    };
    assert_eq!(start(b"m", 1).key(&run), start(b"n", 2).key(&run));
    Ok(())
}

#[test]
fn manifest_a_to_b_to_a_gives_three_keys() {
    let run = run();
    let changed = |manifest: &[u8], from_generation: u32| Event::ManifestChanged {
        manifest: ManifestId::of(manifest),
        from_generation,
    };
    let keys: BTreeSet<EventKey> = [changed(b"a", 1), changed(b"b", 2), changed(b"a", 3)]
        .iter()
        .map(|event| event.key(&run))
        .collect();
    assert_eq!(keys.len(), 3);
}

#[test]
fn stop_requests_are_keyed_by_generation_and_reason() {
    let run = run();
    let stop = |generation: u32, reason: StopReason| Event::StopRequested { generation, reason };
    assert_ne!(
        stop(3, StopReason::UserStop).key(&run),
        stop(5, StopReason::UserStop).key(&run)
    );
    assert_ne!(
        stop(3, StopReason::SpendCap).key(&run),
        stop(3, StopReason::UserStop).key(&run)
    );
    assert_eq!(
        stop(3, StopReason::UserStop).key(&run),
        stop(3, StopReason::UserStop).key(&run)
    );
}

#[test]
fn a_review_request_and_its_answer_have_different_keys() {
    let run = run();
    let request = review(ReviewKind::JudgeDesign, 7);
    let asked = Event::ReviewRequested {
        request,
        kind: ReviewKind::JudgeDesign,
    };
    let answered = Event::Reviewed {
        request,
        answer: ReviewAnswer::Declined,
    };
    assert_ne!(asked.key(&run), answered.key(&run));
}

#[test]
fn review_ids_separate_kinds_subjects_and_runs() {
    let subject = BlobId::of(b"review");
    let params = ReviewId::of(&run(), ReviewKind::Params, &subject);
    assert_ne!(
        params,
        ReviewId::of(&run(), ReviewKind::Dependencies, &subject)
    );
    assert_ne!(
        params,
        ReviewId::of(&run(), ReviewKind::Params, &BlobId::of(b"x"))
    );
    assert_ne!(
        params,
        ReviewId::of(&other_run(), ReviewKind::Params, &subject)
    );
}

#[test]
fn spend_ids_separate_sources() -> TestResult {
    let run = run();
    let model = haiku()?;
    let call = |call: u32| SpendSource::ModelCall {
        call,
        model: model.clone(),
    };
    let ids: BTreeSet<SpendId> = [
        SpendId::of(&run, 1, &call(0)),
        SpendId::of(&run, 1, &call(1)),
        SpendId::of(&run, 2, &call(0)),
        SpendId::of(
            &run,
            1,
            &SpendSource::ModelCall {
                call: 0,
                model: ModelId::from_str("sonnet")?,
            },
        ),
        SpendId::of(
            &run,
            1,
            &SpendSource::Candidate {
                exp: exp(1),
                trial: 0,
            },
        ),
        SpendId::of(
            &run,
            1,
            &SpendSource::Judge {
                exp: exp(1),
                trial: 0,
            },
        ),
        SpendId::of(
            &run,
            1,
            &SpendSource::Candidate {
                exp: exp(1),
                trial: 1,
            },
        ),
        SpendId::of(
            &run,
            1,
            &SpendSource::Candidate {
                exp: exp(2),
                trial: 0,
            },
        ),
        SpendId::of(&other_run(), 1, &call(0)),
    ]
    .into_iter()
    .collect();
    assert_eq!(ids.len(), 9);
    Ok(())
}

#[test]
fn spend_source_names_its_experiment() -> TestResult {
    assert_eq!(
        SpendSource::ModelCall {
            call: 0,
            model: haiku()?
        }
        .exp(),
        None
    );
    assert_eq!(
        SpendSource::Candidate {
            exp: exp(4),
            trial: 1
        }
        .exp(),
        Some(&exp(4))
    );
    assert_eq!(
        SpendSource::Judge {
            exp: exp(5),
            trial: 0
        }
        .exp(),
        Some(&exp(5))
    );
    Ok(())
}

/// A test-side description of an event: small seeds, so key fields collide often.
#[derive(Debug, Clone)]
enum Spec {
    RunStarted {
        manifest: u8,
        started_at: u64,
    },
    ManifestChanged {
        manifest: u8,
        from_generation: u32,
    },
    Proposed {
        exp: u8,
        parent: u8,
        generation: u32,
        rationale: u8,
    },
    Judged {
        exp: u8,
        judge: u8,
        trial: u32,
        crashed: bool,
    },
    Selected {
        generation: u32,
        proposed: u32,
    },
    Spent {
        generation: u32,
        source: u8,
        index: u32,
        exp: u8,
        tokens: u64,
    },
    Steered {
        generation: u32,
        ordinal: u32,
        note: u8,
    },
    StopRequested {
        generation: u32,
        reason: u8,
    },
    ReviewRequested {
        kind: u8,
        subject: u8,
    },
    Reviewed {
        kind: u8,
        subject: u8,
        approved: bool,
    },
    ReportAcknowledged {
        exp: u8,
        report: u8,
    },
}

fn spec_strategy() -> impl Strategy<Value = Spec> {
    let seed = || 0_u8..3;
    let small = || 0_u32..3;
    prop_oneof![
        (seed(), any::<u64>()).prop_map(|(manifest, started_at)| Spec::RunStarted {
            manifest,
            started_at
        }),
        (seed(), small()).prop_map(|(manifest, from_generation)| Spec::ManifestChanged {
            manifest,
            from_generation
        }),
        (seed(), seed(), small(), seed()).prop_map(|(exp, parent, generation, rationale)| {
            Spec::Proposed {
                exp,
                parent,
                generation,
                rationale,
            }
        }),
        (seed(), seed(), small(), any::<bool>()).prop_map(|(exp, judge, trial, crashed)| {
            Spec::Judged {
                exp,
                judge,
                trial,
                crashed,
            }
        }),
        (small(), small()).prop_map(|(generation, proposed)| Spec::Selected {
            generation,
            proposed
        }),
        (small(), 0_u8..3, small(), seed(), any::<u64>()).prop_map(
            |(generation, source, index, exp, tokens)| Spec::Spent {
                generation,
                source,
                index,
                exp,
                tokens
            }
        ),
        (small(), small(), seed()).prop_map(|(generation, ordinal, note)| Spec::Steered {
            generation,
            ordinal,
            note
        }),
        (small(), 0_u8..6)
            .prop_map(|(generation, reason)| Spec::StopRequested { generation, reason }),
        (0_u8..3, seed()).prop_map(|(kind, subject)| Spec::ReviewRequested { kind, subject }),
        (0_u8..3, seed(), any::<bool>()).prop_map(|(kind, subject, approved)| Spec::Reviewed {
            kind,
            subject,
            approved
        }),
        (seed(), seed()).prop_map(|(exp, report)| Spec::ReportAcknowledged { exp, report }),
    ]
}

fn review_kind(seed: u8) -> ReviewKind {
    match seed {
        0 => ReviewKind::JudgeDesign,
        1 => ReviewKind::Params,
        _ => ReviewKind::Dependencies,
    }
}

fn stop_reason(seed: u8) -> StopReason {
    match seed {
        0 => StopReason::UserStop,
        1 => StopReason::SpendCap,
        2 => StopReason::TokenCap,
        3 => StopReason::NoValidCandidates,
        4 => StopReason::InfraFailures,
        _ => StopReason::ProviderLimit,
    }
}

/// The key fields of a spec, written independently of the library: the kind, then each key field.
fn key_fields(spec: &Spec) -> (u8, Vec<u64>) {
    match *spec {
        Spec::RunStarted { .. } => (1, vec![]),
        Spec::ManifestChanged {
            manifest,
            from_generation,
        } => (2, vec![u64::from(manifest), u64::from(from_generation)]),
        Spec::Proposed { exp, .. } => (3, vec![u64::from(exp)]),
        Spec::Judged {
            exp, judge, trial, ..
        } => (4, vec![u64::from(exp), u64::from(judge), u64::from(trial)]),
        Spec::Selected { generation, .. } => (5, vec![u64::from(generation)]),
        Spec::Spent {
            generation,
            source,
            index,
            exp,
            ..
        } => {
            // A model call names no experiment, so its exp seed is not a key field.
            let exp = if source == 0 { 0 } else { u64::from(exp) };
            (
                6,
                vec![
                    u64::from(generation),
                    u64::from(source),
                    u64::from(index),
                    exp,
                ],
            )
        }
        Spec::Steered {
            generation,
            ordinal,
            ..
        } => (7, vec![u64::from(generation), u64::from(ordinal)]),
        Spec::StopRequested { generation, reason } => {
            (8, vec![u64::from(generation), u64::from(reason)])
        }
        Spec::ReviewRequested { kind, subject } => (9, vec![u64::from(kind), u64::from(subject)]),
        Spec::Reviewed { kind, subject, .. } => (10, vec![u64::from(kind), u64::from(subject)]),
        Spec::ReportAcknowledged { exp, report } => (11, vec![u64::from(exp), u64::from(report)]),
    }
}

fn event_of(spec: &Spec) -> Result<Event, Box<dyn Error>> {
    let base = TreeId::of(b"t");
    Ok(match *spec {
        Spec::RunStarted {
            manifest,
            started_at,
        } => Event::RunStarted {
            run: run(),
            manifest: ManifestId::of(&[manifest]),
            base,
            baseline: ExperimentId::baseline(&base),
            started_at,
        },
        Spec::ManifestChanged {
            manifest,
            from_generation,
        } => Event::ManifestChanged {
            manifest: ManifestId::of(&[manifest]),
            from_generation,
        },
        Spec::Proposed {
            exp: e,
            parent,
            generation,
            rationale,
        } => Event::Proposed {
            exp: exp(e),
            parent: exp(parent),
            generation,
            tier: Tier::Sweep,
            diff: BlobId::of(&[b'd', e]),
            rationale: BlobId::of(&[rationale]),
            origin: Origin::Sweep,
        },
        Spec::Judged {
            exp: e,
            judge: j,
            trial,
            crashed,
        } => Event::Judged {
            exp: exp(e),
            judge: judge(j)?,
            trial,
            outcome: if crashed {
                Outcome::Crashed {
                    stage: Stage::Build,
                }
            } else {
                Outcome::TimedOut {
                    stage: Stage::Judge,
                }
            },
            backend: backend(Isolation::Hardware, &EXPECTED_KRUN_LAYERS),
        },
        Spec::Selected {
            generation,
            proposed,
        } => Event::Selected {
            generation,
            decision: Decision::new(DecisionKind::NoValidCandidates, proposed, 0),
        },
        Spec::Spent {
            generation,
            source,
            index,
            exp: e,
            tokens,
        } => Event::Spent {
            generation,
            source: match source {
                0 => SpendSource::ModelCall {
                    call: index,
                    model: haiku()?,
                },
                1 => SpendSource::Candidate {
                    exp: exp(e),
                    trial: index,
                },
                _ => SpendSource::Judge {
                    exp: exp(e),
                    trial: index,
                },
            },
            usage: Usage::tokens(tokens, 0, 0, 0),
        },
        Spec::Steered {
            generation,
            ordinal,
            note,
        } => Event::Steered {
            generation,
            ordinal,
            note: BlobId::of(&[note]),
        },
        Spec::StopRequested { generation, reason } => Event::StopRequested {
            generation,
            reason: stop_reason(reason),
        },
        Spec::ReviewRequested { kind, subject } => Event::ReviewRequested {
            request: review(review_kind(kind), subject),
            kind: review_kind(kind),
        },
        Spec::Reviewed {
            kind,
            subject,
            approved,
        } => Event::Reviewed {
            request: review(review_kind(kind), subject),
            answer: if approved {
                ReviewAnswer::Approved
            } else {
                ReviewAnswer::Declined
            },
        },
        Spec::ReportAcknowledged { exp: e, report } => Event::ReportAcknowledged {
            exp: exp(e),
            report: ReportHash::of(&[report]),
        },
    })
}

proptest! {
    #[test]
    fn distinct_key_fields_give_distinct_keys(specs in prop::collection::vec(spec_strategy(), 0..48)) {
        let run = run();
        let mut by_fields: BTreeMap<(u8, Vec<u64>), EventKey> = BTreeMap::new();
        for spec in &specs {
            let event = event_of(spec).map_err(|error| TestCaseError::fail(error.to_string()))?;
            let key = event.key(&run);
            let fields = key_fields(spec);
            if let Some(seen) = by_fields.get(&fields) {
                prop_assert_eq!(*seen, key, "equal key fields gave different keys: {:?}", spec);
            } else {
                by_fields.insert(fields, key);
            }
        }
        let keys: BTreeSet<EventKey> = by_fields.values().copied().collect();
        prop_assert_eq!(keys.len(), by_fields.len());
    }

    #[test]
    fn every_generated_event_round_trips(spec in spec_strategy()) {
        let event = event_of(&spec).map_err(|error| TestCaseError::fail(error.to_string()))?;
        let record = Record::v1(event);
        let text = serde_json::to_string(&record)?;
        let back: Record = serde_json::from_str(&text)?;
        prop_assert_eq!(back, record);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Record wire shape
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn every_variant_round_trips_under_its_v_tag() -> TestResult {
    for (event, name) in one_of_each()?.into_iter().zip(VARIANT_NAMES) {
        let record = Record::v1(event);
        let text = serde_json::to_string(&record)?;
        assert!(
            text.starts_with(&format!(r#"{{"v":"1","{name}":{{"#)),
            "unexpected shape: {text}"
        );
        let back: Record = serde_json::from_str(&text)?;
        assert_eq!(&back, &record);
        assert_eq!(back.event(), record.event());
    }
    Ok(())
}

#[test]
fn run_started_wire_form() -> TestResult {
    let text = run_started_json()?;
    let run = run().to_hex();
    let manifest = ManifestId::of(b"m").to_hex();
    let base = TreeId::of(b"t").to_hex();
    let baseline = ExperimentId::baseline(&TreeId::of(b"t")).to_hex();
    assert_eq!(
        text,
        format!(
            r#"{{"v":"1","run_started":{{"run":"{run}","manifest":"{manifest}","base":"{base}","baseline":"{baseline}","started_at":1760000000}}}}"#
        )
    );
    Ok(())
}

#[test]
fn record_rejects_another_version() -> TestResult {
    let text = run_started_json()?;
    record_rejected(&text.replacen(r#""v":"1""#, r#""v":"2""#, 1))?;
    record_rejected(&text.replacen(r#""v":"1","#, "", 1))?;
    Ok(())
}

#[test]
fn record_rejects_a_numeric_version_tag() -> TestResult {
    let text = run_started_json()?;
    record_rejected(&text.replacen(r#""v":"1""#, r#""v":1"#, 1))?;
    record_rejected(&text.replacen(r#""v":"1""#, r#""v":0"#, 1))?;
    Ok(())
}

#[test]
fn record_rejects_an_unknown_variant() -> TestResult {
    let text = run_started_json()?;
    record_rejected(&text.replacen("run_started", "run_restarted", 1))?;
    Ok(())
}

#[test]
fn record_rejects_an_extra_field() -> TestResult {
    let text = run_started_json()?;
    record_rejected(&text.replacen(r#""started_at":"#, r#""extra":1,"started_at":"#, 1))?;
    record_rejected(&text.replacen(r#"{"v":"1","#, r#"{"v":"1","x":1,"#, 1))?;
    Ok(())
}

#[test]
fn record_rejects_a_missing_field() -> TestResult {
    let text = run_started_json()?;
    record_rejected(&text.replacen(r#","started_at":1760000000"#, "", 1))?;
    Ok(())
}

#[test]
fn backend_layers_must_be_canonical() -> TestResult {
    let info = backend(
        Isolation::Hardware,
        &[ConfinementLayer::Seatbelt, ConfinementLayer::Rlimits],
    );
    let text = serde_json::to_string(&info)?;
    assert!(
        text.contains(r#""layers":["seatbelt","rlimits"]"#),
        "{text}"
    );
    let back: BackendInfo = serde_json::from_str(&text)?;
    assert_eq!(back, info);
    let duplicated = text.replacen(
        r#""layers":["seatbelt","rlimits"]"#,
        r#""layers":["seatbelt","seatbelt","rlimits"]"#,
        1,
    );
    assert!(serde_json::from_str::<BackendInfo>(&duplicated).is_err());
    let unsorted = text.replacen(
        r#""layers":["seatbelt","rlimits"]"#,
        r#""layers":["rlimits","seatbelt"]"#,
        1,
    );
    assert!(serde_json::from_str::<BackendInfo>(&unsorted).is_err());
    Ok(())
}

#[test]
fn backend_info_accessors() {
    let info = backend(Isolation::Hardware, &[ConfinementLayer::Rlimits]);
    assert_eq!(info.backend(), BackendKind::Krun);
    assert_eq!(info.arch(), Arch::Arm64);
    assert_eq!(info.isolation(), Isolation::Hardware);
    assert_eq!(
        info.layers().iter().copied().collect::<Vec<_>>(),
        vec![ConfinementLayer::Rlimits]
    );
    assert_eq!(info.root_image(), &ImageDigest::of(b"root"));
    assert_eq!(info.kernel(), Some(&ImageDigest::of(b"kernel")));
}

#[test]
fn isolation_orders_weakest_first_and_krun_expects_every_layer() {
    assert!(Isolation::WeakProcess < Isolation::Hardware);
    assert_eq!(
        [Isolation::Hardware, Isolation::WeakProcess]
            .into_iter()
            .min(),
        Some(Isolation::WeakProcess)
    );
    let expected: BTreeSet<ConfinementLayer> = EXPECTED_KRUN_LAYERS.into_iter().collect();
    assert_eq!(
        expected,
        [
            ConfinementLayer::Seatbelt,
            ConfinementLayer::DedicatedUser,
            ConfinementLayer::Rlimits
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn decision_accessors() {
    let decision = Decision::new(DecisionKind::Kept { incumbent: exp(2) }, 5, 2);
    assert_eq!(decision.kind(), &DecisionKind::Kept { incumbent: exp(2) });
    assert_eq!(decision.proposed(), 5);
    assert_eq!(decision.dropped(), 2);
}

// ---------------------------------------------------------------------------------------------------------------
// Line
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn line_verifies_against_its_own_run_only() -> TestResult {
    let run = run();
    for (seq, event) in (0_u64..).zip(one_of_each()?) {
        let key = event.key(&run);
        let line = Line::new(seq, &run, Record::v1(event));
        assert_eq!(line.seq(), seq);
        assert_eq!(line.key(), &key);
        assert_eq!(line.record().event().key(&run), key);
        assert_eq!(line.verify(&run), Ok(()));
        assert_eq!(line.verify(&other_run()), Err(LineError::KeyMismatch));
    }
    Ok(())
}

#[test]
fn line_round_trips_and_rejects_unknown_fields() -> TestResult {
    let run = run();
    let event = one_of_each()?
        .into_iter()
        .next()
        .ok_or("one_of_each is empty")?;
    let line = Line::new(7, &run, Record::v1(event));
    let text = serde_json::to_string(&line)?;
    assert!(text.starts_with(r#"{"seq":7,"key":""#), "{text}");
    let back: Line = serde_json::from_str(&text)?;
    assert_eq!(back, line);
    assert!(
        serde_json::from_str::<Line>(&text.replacen(r#"{"seq":"#, r#"{"extra":1,"seq":"#, 1))
            .is_err()
    );
    Ok(())
}

#[test]
fn a_tampered_record_fails_verification() -> TestResult {
    let run = run();
    let stop = Event::StopRequested {
        generation: 3,
        reason: StopReason::UserStop,
    };
    let line = Line::new(1, &run, Record::v1(stop));
    let text = serde_json::to_string(&line)?;
    let tampered = text.replacen(r#""generation":3"#, r#""generation":4"#, 1);
    assert_ne!(tampered, text);
    let parsed: Line = serde_json::from_str(&tampered)?;
    assert_eq!(parsed.verify(&run), Err(LineError::KeyMismatch));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Closed enums
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn stop_reason_display_strings_are_fixed() {
    let cases = [
        (StopReason::UserStop, "stopped by the user"),
        (StopReason::SpendCap, "spend cap reached"),
        (StopReason::TokenCap, "token cap reached"),
        (
            StopReason::NoValidCandidates,
            "no valid candidate in consecutive generations",
        ),
        (
            StopReason::InfraFailures,
            "consecutive infrastructure failures",
        ),
        (
            StopReason::ProviderLimit,
            "model provider rate or usage limit",
        ),
    ];
    for (reason, text) in cases {
        assert_eq!(reason.to_string(), text);
    }
}

#[test]
fn closed_enums_serialize_as_snake_case() -> TestResult {
    assert_eq!(
        serde_json::to_string(&StopReason::NoValidCandidates)?,
        r#""no_valid_candidates""#
    );
    assert_eq!(
        serde_json::to_string(&ReviewKind::JudgeDesign)?,
        r#""judge_design""#
    );
    assert_eq!(
        serde_json::to_string(&ReviewAnswer::Declined)?,
        r#""declined""#
    );
    assert_eq!(serde_json::to_string(&Origin::Sweep)?, r#""sweep""#);
    assert_eq!(
        serde_json::to_string(&Isolation::WeakProcess)?,
        r#""weak_process""#
    );
    assert_eq!(
        serde_json::to_string(&ConfinementLayer::DedicatedUser)?,
        r#""dedicated_user""#
    );
    assert_eq!(serde_json::to_string(&BackendKind::Krun)?, r#""krun""#);
    assert!(serde_json::from_str::<StopReason>(r#""user_stopped""#).is_err());
    assert!(serde_json::from_str::<ReviewAnswer>(r#""maybe""#).is_err());
    let decision = serde_json::to_string(&Decision::new(DecisionKind::NoValidCandidates, 3, 3))?;
    assert_eq!(
        decision,
        r#"{"kind":"no_valid_candidates","proposed":3,"dropped":3}"#
    );
    assert!(
        serde_json::from_str::<Decision>(
            r#"{"kind":"no_valid_candidates","proposed":3,"dropped":3,"usd":1}"#
        )
        .is_err()
    );
    Ok(())
}
