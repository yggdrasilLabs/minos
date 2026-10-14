//! The manifest: digest-only image references and Dockerfile pins, every leaf type's grammar, manifest-level
//! tier eligibility, every `Manifest::try_from` cross-field rule, and the `JudgeVersion` golden vector.
//!
//! Each rule has its own test that mutates one line of a valid manifest and asserts the exact error variant.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{MetricName, ModelId};
use minos_core::image::{ImageError, ImageRef, base_images};
use minos_core::manifest::{
    ChoiceList, CommandArgv, DurationMs, Eligibility, Goal, IneligibleReason, IntRange,
    MinImprovement, ParamDomain, ParamName, ProjectName, ProposerSpec, SourceExclude, SweepParam,
    Tier,
};
use minos_core::path::RelPath;

type TestResult = Result<(), Box<dyn Error>>;

/// 64 lowercase hex characters: a well-formed digest.
const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// A second well-formed digest that sorts after [`HEX`].
const HEX2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn path(text: &str) -> Result<RelPath, Box<dyn Error>> {
    Ok(RelPath::try_from(text)?)
}

fn argv(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

fn image_rejected(text: &str) -> Result<ImageError, Box<dyn Error>> {
    match ImageRef::from_str(text) {
        Ok(image) => Err(format!("{text:?} parsed as {image}").into()),
        Err(error) => Ok(error),
    }
}

fn dockerfile_rejected(text: &str) -> Result<ImageError, Box<dyn Error>> {
    match base_images(text) {
        Ok(refs) => Err(format!("the Dockerfile was accepted with {} refs", refs.len()).into()),
        Err(error) => Ok(error),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// ImageRef
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn image_ref_parses_a_docker_hub_name_with_a_digest() -> TestResult {
    let text = format!("docker.io/library/alpine@sha256:{HEX}");
    let image = ImageRef::from_str(&text)?;
    assert_eq!(image.name(), "docker.io/library/alpine");
    assert_eq!(image.digest().to_hex(), HEX);
    assert_eq!(image.to_string(), text);
    Ok(())
}

#[test]
fn image_ref_allows_a_registry_port() -> TestResult {
    let image = ImageRef::from_str(&format!("localhost:5000/team/img@sha256:{HEX}"))?;
    assert_eq!(image.name(), "localhost:5000/team/img");
    Ok(())
}

#[test]
fn image_ref_rejects_a_tag() -> TestResult {
    assert!(matches!(image_rejected("alpine:3.20")?, ImageError::Tag));
    Ok(())
}

#[test]
fn image_ref_rejects_a_tag_beside_a_digest() -> TestResult {
    let error = image_rejected(&format!("alpine:3.20@sha256:{HEX}"))?;
    assert!(matches!(error, ImageError::Tag), "got: {error}");
    Ok(())
}

#[test]
fn image_ref_rejects_a_missing_digest() -> TestResult {
    assert!(matches!(
        image_rejected("alpine")?,
        ImageError::MissingDigest
    ));
    Ok(())
}

#[test]
fn image_ref_rejects_a_short_digest() -> TestResult {
    let short = HEX.get(1..).ok_or("short hex")?;
    let error = image_rejected(&format!("alpine@sha256:{short}"))?;
    assert!(matches!(error, ImageError::BadDigest), "got: {error}");
    Ok(())
}

#[test]
fn image_ref_rejects_uppercase_hex() -> TestResult {
    let error = image_rejected(&format!("alpine@sha256:{}", HEX.to_ascii_uppercase()))?;
    assert!(matches!(error, ImageError::BadDigest), "got: {error}");
    Ok(())
}

#[test]
fn image_ref_rejects_an_empty_name() -> TestResult {
    let error = image_rejected(&format!("@sha256:{HEX}"))?;
    assert!(matches!(error, ImageError::BadName), "got: {error}");
    Ok(())
}

#[test]
fn image_ref_round_trips_through_serde() -> TestResult {
    let text = format!("\"alpine@sha256:{HEX}\"");
    let image: ImageRef = serde_json::from_str(&text)?;
    assert_eq!(serde_json::to_string(&image)?, text);
    assert!(serde_json::from_str::<ImageRef>("\"alpine:3.20\"").is_err());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// base_images
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn base_images_accepts_pins_scratch_and_stage_aliases() -> TestResult {
    let dockerfile = format!(
        "# a comment\n\
         FROM --platform=linux/arm64 rust@sha256:{HEX2} AS Build\n\
         RUN cargo build --release \\\n    --locked\n\
         \n\
         from \\\n  alpine@sha256:{HEX}\n\
         FROM build AS again\n\
         FROM scratch\n\
         FROM alpine@sha256:{HEX}\n"
    );
    let refs = base_images(&dockerfile)?;
    let shown: Vec<String> = refs.iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        vec![
            format!("alpine@sha256:{HEX}"),
            format!("rust@sha256:{HEX2}")
        ]
    );
    Ok(())
}

#[test]
fn base_images_rejects_a_tagged_from() -> TestResult {
    let error = dockerfile_rejected("# base\nFROM alpine:3.20\n")?;
    assert!(
        matches!(error, ImageError::Unpinned { line: 2 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_rejects_a_variable_from() -> TestResult {
    let error = dockerfile_rejected("ARG BASE\nFROM $BASE\n")?;
    assert!(
        matches!(error, ImageError::Variable { line: 2 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_rejects_a_braced_variable_with_a_digest() -> TestResult {
    let error = dockerfile_rejected(&format!("FROM ${{BASE}}@sha256:{HEX}\n"))?;
    assert!(
        matches!(error, ImageError::Variable { line: 1 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_rejects_a_from_with_no_image() -> TestResult {
    let error = dockerfile_rejected("FROM --platform=linux/arm64\n")?;
    assert!(
        matches!(error, ImageError::EmptyFrom { line: 1 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_reports_the_first_line_of_a_continued_from() -> TestResult {
    let error = dockerfile_rejected("RUN true\nFROM \\\n  alpine:3.20\n")?;
    assert!(
        matches!(error, ImageError::Unpinned { line: 2 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_skips_comments_inside_a_continuation() -> TestResult {
    // Docker drops comment lines inside a continued instruction, so `FROM` here is an argument of RUN.
    let refs = base_images("RUN echo \\\n# note\n  FROM alpine:3.20\n")?;
    assert!(refs.is_empty());
    Ok(())
}

#[test]
fn base_images_rejects_a_flag_without_a_value() -> TestResult {
    let error = dockerfile_rejected(&format!(
        "FROM --platform linux/arm64 alpine@sha256:{HEX}\n"
    ))?;
    assert!(
        matches!(error, ImageError::MalformedFrom { line: 1 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_rejects_a_changed_escape_character() -> TestResult {
    let error = dockerfile_rejected(&format!("# escape=`\nFROM alpine@sha256:{HEX}\n"))?;
    assert!(
        matches!(error, ImageError::EscapeDirective { line: 1 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn base_images_rejects_an_unpinned_syntax_frontend() -> TestResult {
    let error = dockerfile_rejected(&format!(
        "# syntax=docker/dockerfile:1\nFROM alpine@sha256:{HEX}\n"
    ))?;
    assert!(
        matches!(error, ImageError::Unpinned { line: 1 }),
        "got: {error}"
    );
    let pinned = format!("# syntax=docker/dockerfile@sha256:{HEX2}\nFROM alpine@sha256:{HEX}\n");
    assert_eq!(base_images(&pinned)?.len(), 1);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn project_name_grammar() -> TestResult {
    assert_eq!(ProjectName::from_str("logparse")?.as_str(), "logparse");
    assert!(ProjectName::from_str("0-a_b").is_ok());
    for bad in ["", "Logparse", "-x", "_x", "a b", "a.b"] {
        assert!(ProjectName::from_str(bad).is_err(), "{bad:?} accepted");
    }
    assert!(ProjectName::from_str(&"a".repeat(64)).is_ok());
    assert!(ProjectName::from_str(&"a".repeat(65)).is_err());
    Ok(())
}

#[test]
fn param_name_grammar() -> TestResult {
    assert_eq!(ParamName::from_str("chunk_size")?.as_str(), "chunk_size");
    for bad in ["", "1x", "_x", "Chunk", "chunk-size"] {
        assert!(ParamName::from_str(bad).is_err(), "{bad:?} accepted");
    }
    assert!(ParamName::from_str(&"a".repeat(65)).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// CommandArgv
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn command_argv_rejects_an_empty_array() {
    assert!(CommandArgv::try_from(Vec::new()).is_err());
}

#[test]
fn command_argv_rejects_an_empty_first_entry() {
    assert!(CommandArgv::try_from(argv(&["", "x"])).is_err());
}

#[test]
fn command_argv_rejects_a_nul_byte() {
    assert!(CommandArgv::try_from(argv(&["judge", "a\0b"])).is_err());
}

#[test]
fn command_argv_rejects_65_entries() {
    assert!(CommandArgv::try_from(vec!["x".to_owned(); 64]).is_ok());
    assert!(CommandArgv::try_from(vec!["x".to_owned(); 65]).is_err());
}

#[test]
fn command_argv_rejects_a_4097_byte_entry() {
    assert!(CommandArgv::try_from(vec!["judge".to_owned(), "a".repeat(4096)]).is_ok());
    assert!(CommandArgv::try_from(vec!["judge".to_owned(), "a".repeat(4097)]).is_err());
}

#[test]
fn command_argv_program_paths() -> TestResult {
    let dot = CommandArgv::try_from(argv(&["./judge"]))?;
    assert_eq!(dot.program(), Some(&path("judge")?));
    let nested = CommandArgv::try_from(argv(&["judge/run.sh", "--fast"]))?;
    assert_eq!(nested.program(), Some(&path("judge/run.sh")?));
    assert_eq!(
        nested.as_slice(),
        argv(&["judge/run.sh", "--fast"]).as_slice()
    );
    assert_eq!(
        CommandArgv::try_from(argv(&["/usr/bin/python3"]))?.program(),
        None
    );
    assert_eq!(CommandArgv::try_from(argv(&["python3"]))?.program(), None);
    assert!(CommandArgv::try_from(argv(&["./../x"])).is_err());
    assert!(CommandArgv::try_from(argv(&["a/../x"])).is_err());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// DurationMs
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn duration_parses_seconds_minutes_and_hours() -> TestResult {
    assert_eq!(DurationMs::parse("2m")?.as_ms(), 120_000);
    assert_eq!(DurationMs::parse("90s")?.as_ms(), 90_000);
    assert_eq!(DurationMs::parse("1h")?.as_ms(), 3_600_000);
    assert_eq!(DurationMs::parse("1s")?.as_ms(), 1_000);
    Ok(())
}

#[test]
fn duration_rejects_out_of_range_and_malformed_text() {
    for bad in [
        "0s", "61m", "2", "2 m", "-1m", "1.5m", "2min", "", "m", "1234567s", "2h",
    ] {
        assert!(DurationMs::parse(bad).is_err(), "{bad:?} accepted");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// MinImprovement and Goal
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn min_improvement_parses_a_percentage_as_a_ratio() -> TestResult {
    let five = MinImprovement::parse("5%")?;
    assert!(five.is_relative());
    assert_eq!(five.amount().to_bits(), 0.05_f64.to_bits());
    assert_eq!(five.to_string(), "5%");
    let tenth = MinImprovement::parse("0.1%")?;
    assert_eq!(tenth.amount().to_bits(), 0.001_f64.to_bits());
    Ok(())
}

#[test]
fn min_improvement_parses_an_absolute_amount() -> TestResult {
    let quarter = MinImprovement::parse("0.25")?;
    assert!(!quarter.is_relative());
    assert_eq!(quarter.amount().to_bits(), 0.25_f64.to_bits());
    assert_eq!(quarter.to_string(), "0.25");
    let zero = MinImprovement::parse("0%")?;
    assert_eq!(zero.amount().to_bits(), 0.0_f64.to_bits());
    assert!(MinImprovement::parse("1000%").is_ok());
    Ok(())
}

#[test]
fn min_improvement_rejects_bad_text() {
    for bad in [
        "1001%", "-1%", "1e3", "abc", "", "%", ".5", "5.", "5 %", "+5%", "1.2.3", "5%%",
    ] {
        assert!(MinImprovement::parse(bad).is_err(), "{bad:?} accepted");
    }
}

#[test]
fn min_improvement_threshold_never_divides() -> TestResult {
    let five = MinImprovement::parse("5%")?;
    assert_eq!(
        five.threshold_at(200.0).map(f64::to_bits),
        Some(10.0_f64.to_bits())
    );
    assert_eq!(
        five.threshold_at(-200.0).map(f64::to_bits),
        Some(10.0_f64.to_bits())
    );
    assert_eq!(
        five.threshold_at(0.0).map(f64::to_bits),
        Some(0.0_f64.to_bits())
    );
    assert_eq!(five.threshold_at(f64::INFINITY), None);
    let quarter = MinImprovement::parse("0.25")?;
    assert_eq!(
        quarter.threshold_at(1e9).map(f64::to_bits),
        Some(0.25_f64.to_bits())
    );
    Ok(())
}

#[test]
fn goal_orients_a_delta() {
    assert_eq!(Goal::Maximize.oriented(2.0).to_bits(), 2.0_f64.to_bits());
    assert_eq!(Goal::Minimize.oriented(2.0).to_bits(), (-2.0_f64).to_bits());
}

// ---------------------------------------------------------------------------------------------------------------
// Tiers, proposers, and eligibility
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn tier_rank_follows_declaration_order() {
    assert_eq!(Tier::Sweep.rank(), 0);
    assert_eq!(Tier::SingleShot.rank(), 1);
    assert_eq!(Tier::Planned.rank(), 2);
    assert_eq!(Tier::Agentic.rank(), 3);
    assert!(Tier::Sweep < Tier::SingleShot && Tier::Planned < Tier::Agentic);
}

#[test]
fn tier_serializes_in_snake_case() -> TestResult {
    assert_eq!(serde_json::to_string(&Tier::SingleShot)?, "\"single_shot\"");
    assert!(matches!(
        serde_json::from_str::<Tier>("\"agentic\"")?,
        Tier::Agentic
    ));
    Ok(())
}

fn sample_param() -> Result<SweepParam, Box<dyn Error>> {
    let domain = ParamDomain::Int(IntRange::new(1, 64, 1)?);
    Ok(SweepParam::new(
        ParamName::from_str("chunk")?,
        path("src/parse.rs")?,
        "const CHUNK: usize = ".to_owned(),
        domain,
    )?)
}

#[test]
fn sweep_eligibility_needs_a_param() -> TestResult {
    let sweep = ProposerSpec::new(Tier::Sweep, None, 4)?;
    assert!(matches!(
        sweep.declared_eligibility(&[]),
        Eligibility::Ineligible(IneligibleReason::NoParams)
    ));
    assert!(matches!(
        sweep.declared_eligibility(&[sample_param()?]),
        Eligibility::Eligible
    ));
    Ok(())
}

#[test]
fn single_shot_eligibility_needs_a_model() -> TestResult {
    let bare = ProposerSpec::new(Tier::SingleShot, None, 4)?;
    assert!(matches!(
        bare.declared_eligibility(&[]),
        Eligibility::Ineligible(IneligibleReason::NoModel)
    ));
    let modelled = ProposerSpec::new(Tier::SingleShot, Some(ModelId::from_str("mid")?), 4)?;
    assert!(matches!(
        modelled.declared_eligibility(&[]),
        Eligibility::Eligible
    ));
    Ok(())
}

#[test]
fn planned_and_agentic_are_not_in_this_release() -> TestResult {
    let model = ModelId::from_str("mid")?;
    for tier in [Tier::Planned, Tier::Agentic] {
        let spec = ProposerSpec::new(tier, Some(model.clone()), 1)?;
        assert!(matches!(
            spec.declared_eligibility(&[sample_param()?]),
            Eligibility::Ineligible(IneligibleReason::NotInThisRelease)
        ));
    }
    Ok(())
}

#[test]
fn proposer_rejects_a_batch_out_of_range_and_a_sweep_with_a_model() -> TestResult {
    assert!(ProposerSpec::new(Tier::SingleShot, None, 0).is_err());
    assert!(ProposerSpec::new(Tier::SingleShot, None, 17).is_err());
    assert!(ProposerSpec::new(Tier::Sweep, Some(ModelId::from_str("mid")?), 1).is_err());
    Ok(())
}

#[test]
fn ineligible_reasons_have_fixed_text() {
    for reason in [
        IneligibleReason::NoParams,
        IneligibleReason::NoModel,
        IneligibleReason::NotInThisRelease,
        IneligibleReason::SurfaceTooLarge {
            bytes: 300_000,
            limit: 262_144,
        },
    ] {
        assert!(!reason.to_string().is_empty());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Sweep params
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn int_range_counts_and_indexes_points() -> TestResult {
    let range = IntRange::new(-4, 4, 3)?;
    assert_eq!(range.points(), 3);
    assert_eq!(range.value_at(0), Some(-4));
    assert_eq!(range.value_at(2), Some(2));
    assert_eq!(range.value_at(3), None);
    let single = IntRange::new(7, 7, 1)?;
    assert_eq!(single.points(), 1);
    Ok(())
}

#[test]
fn int_range_rejects_bad_bounds() {
    assert!(IntRange::new(2, 1, 1).is_err(), "min > max accepted");
    assert!(IntRange::new(1, 2, 0).is_err(), "step 0 accepted");
    assert!(IntRange::new(1, 2, -1).is_err(), "negative step accepted");
    assert!(IntRange::new(0, 999_999, 1).is_ok());
    assert!(
        IntRange::new(0, 1_000_000, 1).is_err(),
        "over MAX_PARAM_POINTS accepted"
    );
    assert!(IntRange::new(i64::MIN, i64::MAX, 1).is_err());
}

#[test]
fn choice_list_rules() -> TestResult {
    let list = ChoiceList::new(argv(&["fast", "small"]))?;
    assert_eq!(list.len(), 2);
    assert_eq!(list.get(1), Some("small"));
    assert_eq!(list.get(2), None);
    assert!(ChoiceList::new(Vec::new()).is_err(), "empty accepted");
    assert!(
        ChoiceList::new(argv(&["a", "a"])).is_err(),
        "duplicate accepted"
    );
    assert!(
        ChoiceList::new(argv(&["a\nb"])).is_err(),
        "newline accepted"
    );
    assert!(
        ChoiceList::new(argv(&[""])).is_err(),
        "empty choice accepted"
    );
    assert!(
        ChoiceList::new(vec!["a".repeat(257)]).is_err(),
        "long choice accepted"
    );
    let many: Vec<String> = (0..65).map(|index| format!("c{index}")).collect();
    assert!(ChoiceList::new(many).is_err(), "65 choices accepted");
    Ok(())
}

#[test]
fn sweep_param_anchor_rules() -> TestResult {
    let make = |anchor: String| -> Result<SweepParam, Box<dyn Error>> {
        Ok(SweepParam::new(
            ParamName::from_str("chunk")?,
            path("src/parse.rs")?,
            anchor,
            ParamDomain::Choices(ChoiceList::new(argv(&["1", "2"]))?),
        )?)
    };
    let param = make("let chunk = ".to_owned())?;
    assert_eq!(param.anchor(), "let chunk = ");
    assert_eq!(param.file(), &path("src/parse.rs")?);
    assert_eq!(param.name().as_str(), "chunk");
    assert!(matches!(param.domain(), ParamDomain::Choices(list) if list.len() == 2));
    assert!(make(String::new()).is_err(), "empty anchor accepted");
    assert!(
        make("a\tb".to_owned()).is_err(),
        "control character accepted"
    );
    assert!(make("a".repeat(257)).is_err(), "long anchor accepted");
    assert!(make("a".repeat(256)).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// SourceExclude
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn source_exclude_covers_case_insensitively_at_component_boundaries() -> TestResult {
    let judge = SourceExclude::parse("judge/")?;
    assert!(judge.is_dir());
    assert_eq!(judge.path(), &path("judge")?);
    assert_eq!(judge.to_string(), "judge/");
    assert!(judge.covers(&path("judge/x.py")?));
    assert!(judge.covers(&path("Judge/x.py")?));
    assert!(judge.covers(&path("judge")?));
    assert!(!judge.covers(&path("judgement.py")?));
    assert!(!judge.covers(&path("src/judge/x.py")?));
    let file = SourceExclude::parse("notes.txt")?;
    assert!(!file.is_dir());
    assert!(file.covers(&path("NOTES.txt")?));
    for bad in ["", "/", "judge//", "/judge", "../x", "a/./b"] {
        assert!(SourceExclude::parse(bad).is_err(), "{bad:?} accepted");
    }
    Ok(())
}

#[test]
fn metric_names_used_in_the_manifest_are_core_names() -> TestResult {
    assert_eq!(
        MetricName::from_str("throughput_mb_s")?.as_str(),
        "throughput_mb_s"
    );
    Ok(())
}
