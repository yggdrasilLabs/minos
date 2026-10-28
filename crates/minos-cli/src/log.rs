//! `minos log [--run <prefix>]`: one line per ledger event in `seq` order, with IDs as 12-hex prefixes and enums by
//! name, filterable by generation and experiment, and `--json` for the stored lines (CLI-07; 08-CONTEXT D-16).
//!
//! [`log`] reads the run's verified lines and writes, for each line it keeps, `<seq> <kind> <generation> <fields>`,
//! where the kind is the event's snake-case name, the generation is `-` for a review event, and the fields are
//! `name=value` pairs of IDs (short unless [`IdStyle::Full`]), numbers, and closed-set names:
//!
//! | Kind | Fields |
//! |---|---|
//! | `run_started` | `run= manifest= base= baseline= root_image=` |
//! | `manifest_changed` | `manifest= from_generation=` |
//! | `proposed` | `exp= parent= tier= origin=` (`sweep` or `model:<model id>`) |
//! | `judged` | `exp= trial= judge= outcome=` |
//! | `selected` | `decision=` and `winner=` or `incumbent=` |
//! | `spent` | `source=` (`model_call call= model=`, `candidate exp= trial=`, or `judge exp= trial=`) and `in= out= vcpu_ms=` |
//! | `steered` | `ordinal= note=` |
//! | `stop_requested` | `reason=` and `ordinal=` when it has one |
//! | `review_requested`, `reviewed` | `request=` and the kind or answer |
//! | `report_acknowledged` | `exp= report=` |
//!
//! An event's generation is its own `generation` field; `run_started` is generation 0; `manifest_changed` is the
//! generation it takes effect from; and `judged` and `report_acknowledged` are the generation of the experiment they
//! name, 0 for the baseline. Review events have none, so a `--generation` filter drops them. An `--exp` filter keeps
//! the events that name the experiment: its `proposed`, `judged`, candidate and judge `spent`, and
//! `report_acknowledged` lines, and a `selected` line whose decision names it.
//!
//! `--json` writes each kept line as `serde_json` encodes the verified [`Line`], which reproduces the bytes the store
//! wrote, so with no filter the output equals the run's `ledger.jsonl` (D-16). A ledger line holds only IDs,
//! numbers, and closed enums by construction (rationales, notes, and logs are blobs); every line still passes
//! [`write_line`], which leaves such a line unchanged.
//!
//! `log` reads only: it never appends to the ledger, never takes the run lock, and creates nothing. A ledger the
//! store refuses is an error naming the line and its kind.

use std::collections::BTreeMap;
use std::io::{self, Write};

use minos_core::id::ExperimentId;
use minos_core::ledger::{
    DecisionKind, Event, Line, Origin, ReviewAnswer, ReviewKind, SpendSource,
};
use minos_loop::record::event_kind;
use minos_store::{Ledger, StoreError};

use crate::address::{AddressError, HexPrefix, resolve_run};
use crate::render::{IdStyle, decision_word, id_text, outcome_word, stop_reason_name, write_line};

/// What `log` keeps and how it prints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogOptions {
    generation: Option<u32>,
    exp: Option<HexPrefix>,
    json: bool,
    ids: IdStyle,
}

impl LogOptions {
    /// Options that keep only `generation`'s events (`--generation`) and only the events naming the experiment
    /// `exp` names (`--exp`), print the stored lines when `json` (`--json`), and print IDs in `ids` style
    /// (`--full-ids`).
    #[must_use]
    pub const fn new(
        generation: Option<u32>,
        exp: Option<HexPrefix>,
        json: bool,
        ids: IdStyle,
    ) -> Self {
        Self {
            generation,
            exp,
            json,
            ids,
        }
    }
}

/// Why `log` could not print. Messages carry IDs, line numbers, and fixed text only.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// The run or the experiment could not be named.
    #[error("the run or experiment could not be named")]
    Address(#[source] AddressError),
    /// The ledger could not be read or was refused; a refusal names the line and its kind.
    #[error("the ledger could not be read")]
    Store(#[source] StoreError),
    /// A line could not be encoded as JSON.
    #[error("a ledger line could not be encoded")]
    Encode(#[source] serde_json::Error),
    /// The output could not be written.
    #[error("could not write the output")]
    Output(#[source] io::Error),
}

/// Writes the events of the run `run` names in `store`, or of the latest run, to `out`, filtered and formatted as
/// `options` asks and the [module docs](self) describe.
///
/// # Errors
///
/// [`LogError::Address`] when the run cannot be named or the `--exp` prefix matches no experiment or several;
/// [`LogError::Store`] when the ledger cannot be read, including [`StoreError::Corrupt`], which names the line;
/// [`LogError::Encode`] when a line cannot be encoded; and [`LogError::Output`] when `out` cannot be written.
pub fn log<L: Ledger, W: Write>(
    store: &L,
    run: Option<&HexPrefix>,
    options: LogOptions,
    out: &mut W,
) -> Result<(), LogError> {
    let LogOptions {
        generation: wanted,
        exp,
        json,
        ids,
    } = options;
    let resolved = resolve_run(store, run).map_err(LogError::Address)?;
    let read = store.read(resolved.run()).map_err(LogError::Store)?;
    let generations = experiment_generations(read.lines());
    let exp = exp
        .map(|prefix| named_exp(&generations, &prefix))
        .transpose()?;

    for line in read.lines() {
        let event = line.record().event();
        let generation = generation_of(event, &generations);
        if wanted.is_some_and(|wanted| generation != Some(wanted)) {
            continue;
        }
        if exp.is_some_and(|exp| !names(event, &exp)) {
            continue;
        }
        let text = if json {
            let bytes = serde_json::to_vec(line).map_err(LogError::Encode)?;
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            let generation = generation.map_or_else(|| "-".to_owned(), |g| g.to_string());
            let fields = fields(event, ids);
            let head = format!("{} {} {generation}", line.seq(), event_kind(event));
            if fields.is_empty() {
                head
            } else {
                format!("{head} {fields}")
            }
        };
        write_line(out, &text).map_err(LogError::Output)?;
    }
    Ok(())
}

/// The generation of every experiment the lines name: 0 for the baseline, and each proposal's generation.
fn experiment_generations(lines: &[Line]) -> BTreeMap<ExperimentId, u32> {
    lines
        .iter()
        .filter_map(|line| match line.record().event() {
            Event::RunStarted { baseline, .. } => Some((*baseline, 0)),
            Event::Proposed {
                exp, generation, ..
            } => Some((*exp, *generation)),
            _ => None,
        })
        .collect()
}

/// The one experiment `prefix` names among `generations`.
fn named_exp(
    generations: &BTreeMap<ExperimentId, u32>,
    prefix: &HexPrefix,
) -> Result<ExperimentId, LogError> {
    let matched: Vec<ExperimentId> = generations
        .keys()
        .filter(|exp| prefix.matches(&exp.to_hex()))
        .copied()
        .collect();
    match matched.as_slice() {
        [] => Err(LogError::Address(AddressError::NoExperiment {
            prefix: prefix.clone(),
        })),
        [exp] => Ok(*exp),
        _ => Err(LogError::Address(AddressError::Ambiguous {
            what: "experiment",
            prefix: prefix.clone(),
            candidates: matched.iter().map(ExperimentId::to_hex).collect(),
        })),
    }
}

/// The generation `event` belongs to, as the [module docs](self) define it; `None` for a review event.
fn generation_of(event: &Event, generations: &BTreeMap<ExperimentId, u32>) -> Option<u32> {
    match event {
        Event::RunStarted { .. } => Some(0),
        Event::ManifestChanged {
            from_generation, ..
        } => Some(*from_generation),
        Event::Proposed { generation, .. }
        | Event::Selected { generation, .. }
        | Event::Spent { generation, .. }
        | Event::Steered { generation, .. }
        | Event::StopRequested { generation, .. } => Some(*generation),
        Event::Judged { exp, .. } | Event::ReportAcknowledged { exp, .. } => {
            generations.get(exp).copied()
        }
        _ => None,
    }
}

/// Whether `event` names `exp`, as the `--exp` filter keeps it.
fn names(event: &Event, exp: &ExperimentId) -> bool {
    match event {
        Event::Proposed { exp: named, .. }
        | Event::Judged { exp: named, .. }
        | Event::ReportAcknowledged { exp: named, .. } => named == exp,
        Event::Spent { source, .. } => source.exp() == Some(exp),
        Event::Selected { decision, .. } => match decision.kind() {
            DecisionKind::Improved { winner } => winner == exp,
            DecisionKind::Kept { incumbent } => incumbent == exp,
            DecisionKind::NoValidCandidates => false,
        },
        _ => false,
    }
}

/// The `name=value` fields of `event`, as the [module docs](self) table lists them.
fn fields(event: &Event, ids: IdStyle) -> String {
    let id = |hex: String| id_text(&hex, ids);
    match event {
        Event::RunStarted {
            run,
            manifest,
            base,
            baseline,
            root_image,
            ..
        } => format!(
            "run={} manifest={} base={} baseline={} root_image={}",
            id(run.to_hex()),
            id(manifest.to_hex()),
            id(base.to_hex()),
            id(baseline.to_hex()),
            root_image.map_or_else(|| "-".to_owned(), |image| id(image.to_hex()))
        ),
        Event::ManifestChanged {
            manifest,
            from_generation,
        } => format!(
            "manifest={} from_generation={from_generation}",
            id(manifest.to_hex())
        ),
        Event::Proposed {
            exp,
            parent,
            tier,
            origin,
            ..
        } => {
            let origin = match origin {
                Origin::Sweep => "sweep".to_owned(),
                Origin::Model { model, .. } => format!("model:{model}"),
            };
            format!(
                "exp={} parent={} tier={} origin={origin}",
                id(exp.to_hex()),
                id(parent.to_hex()),
                tier.as_str()
            )
        }
        Event::Judged {
            exp,
            judge,
            trial,
            outcome,
            ..
        } => format!(
            "exp={} trial={trial} judge={} outcome={}",
            id(exp.to_hex()),
            id(judge.to_hex()),
            outcome_word(outcome)
        ),
        Event::Selected { decision, .. } => {
            let word = decision_word(decision);
            match decision.kind() {
                DecisionKind::Improved { winner } => {
                    format!("decision={word} winner={}", id(winner.to_hex()))
                }
                DecisionKind::Kept { incumbent } => {
                    format!("decision={word} incumbent={}", id(incumbent.to_hex()))
                }
                DecisionKind::NoValidCandidates => format!("decision={word}"),
            }
        }
        Event::Spent { source, usage, .. } => {
            let source = match source {
                SpendSource::ModelCall { call, model } => {
                    format!("model_call call={call} model={model}")
                }
                SpendSource::Candidate { exp, trial } => {
                    format!("candidate exp={} trial={trial}", id(exp.to_hex()))
                }
                SpendSource::Judge { exp, trial } => {
                    format!("judge exp={} trial={trial}", id(exp.to_hex()))
                }
            };
            let input = usage
                .input_tokens()
                .saturating_add(usage.cache_creation_input_tokens())
                .saturating_add(usage.cache_read_input_tokens());
            format!(
                "source={source} in={input} out={} vcpu_ms={}",
                usage.output_tokens(),
                usage.vcpu_ms()
            )
        }
        Event::Steered { ordinal, note, .. } => {
            format!("ordinal={ordinal} note={}", id(note.to_hex()))
        }
        Event::StopRequested {
            reason, ordinal, ..
        } => {
            let ordinal = ordinal.map_or_else(String::new, |ordinal| format!(" ordinal={ordinal}"));
            format!("reason={}{ordinal}", stop_reason_name(*reason))
        }
        Event::ReviewRequested { request, kind } => {
            format!(
                "request={} kind={}",
                id(request.to_hex()),
                review_kind(*kind)
            )
        }
        Event::Reviewed { request, answer } => format!(
            "request={} answer={}",
            id(request.to_hex()),
            review_answer(*answer)
        ),
        Event::ReportAcknowledged { exp, report } => {
            format!("exp={} report={}", id(exp.to_hex()), id(report.to_hex()))
        }
        // `Event` is non-exhaustive; a variant added later prints its kind and no fields.
        _ => String::new(),
    }
}

/// The review kind's snake-case name; a variant this build does not know prints as `other`.
const fn review_kind(kind: ReviewKind) -> &'static str {
    match kind {
        ReviewKind::JudgeDesign => "judge_design",
        ReviewKind::Params => "params",
        ReviewKind::Dependencies => "dependencies",
        _ => "other",
    }
}

/// The review answer's snake-case name; a variant this build does not know prints as `other`.
const fn review_answer(answer: ReviewAnswer) -> &'static str {
    match answer {
        ReviewAnswer::Approved => "approved",
        ReviewAnswer::Declined => "declined",
        _ => "other",
    }
}
