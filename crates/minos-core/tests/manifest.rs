//! The manifest: digest-only image references and Dockerfile pins, every leaf type's grammar, manifest-level
//! tier eligibility, every `Manifest::try_from` cross-field rule, and the `JudgeVersion` golden vector.
//!
//! Each rule has its own test that mutates one line of a valid manifest and asserts the exact error variant.

use std::error::Error;
use std::str::FromStr;

use minos_core::id::{MetricName, ModelId, TreeId};
use minos_core::image::{ImageError, ImageRef, base_images};
use minos_core::limits::{
    DEFAULT_JUDGE_MS, DEFAULT_MEMORY_MIB, DEFAULT_SCRATCH_MIB, EMPTY_GENERATIONS_DEFAULT,
    GUEST_RESERVE_MIB, INFRA_FAILURES_DEFAULT, MAX_MEMORY_MIB, MAX_SCRATCH_MIB,
    MIN_CANDIDATE_ALLOWANCE_MIB, MIN_MEMORY_MIB, MIN_SCRATCH_MIB, STALL_STEP_DEFAULT,
    TOKEN_CAP_DEFAULT, candidate_allowance_mib,
};
use minos_core::manifest::{
    Arch, ChoiceList, CommandArgv, DurationMs, Eligibility, Goal, IneligibleReason, IntRange,
    JudgeVersion, Manifest, ManifestError, MinImprovement, ParamDomain, ParamName, ProjectKind,
    ProjectName, ProposerSpec, SourceExclude, SweepParam, Tier,
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

#[test]
fn base_images_sees_a_from_behind_a_byte_order_mark() -> TestResult {
    // Docker strips a UTF-8 BOM from the first line, so this is an unpinned FROM to Docker; it must be to us.
    let error = dockerfile_rejected("\u{feff}FROM alpine:3.20\nRUN true\n")?;
    assert!(
        matches!(error, ImageError::Unpinned { line: 1 }),
        "got: {error}"
    );
    let pinned = format!("\u{feff}FROM alpine@sha256:{HEX}\n");
    assert_eq!(base_images(&pinned)?.len(), 1);
    Ok(())
}

#[test]
fn base_images_sees_a_from_behind_unicode_whitespace() -> TestResult {
    // Docker trims leading whitespace with `unicode.IsSpace`: NBSP, NEL, vertical tab, and form feed included.
    for prefix in [
        "\u{00a0}",
        "\u{0085}",
        "\u{000b}",
        "\u{000c}",
        " \u{00a0}\t",
    ] {
        let error = dockerfile_rejected(&format!("RUN true\n{prefix}FROM alpine:3.20\n"))?;
        assert!(
            matches!(error, ImageError::Unpinned { line: 2 }),
            "prefix {prefix:?}: got {error}"
        );
    }
    let pinned = format!("\u{00a0}FROM alpine@sha256:{HEX}\n");
    assert_eq!(base_images(&pinned)?.len(), 1);
    Ok(())
}

#[test]
fn base_images_rejects_an_unknown_instruction() -> TestResult {
    // An unknown first word is an error, never a skip, so a keyword the parser does not recognise cannot hide a
    // FROM; this also refuses heredoc bodies, whose lines are data to Docker.
    for text in [
        "FORM alpine:3.20\n",
        "RUN true\n\u{00a0}ENVX=1\n",
        "RUN <<EOF\napt-get update\nEOF\n",
    ] {
        let error = dockerfile_rejected(text)?;
        assert!(
            matches!(error, ImageError::UnknownInstruction { .. }),
            "text {text:?}: got {error}"
        );
    }
    let error = dockerfile_rejected("RUN true\nFORM alpine:3.20\n")?;
    assert!(
        matches!(error, ImageError::UnknownInstruction { line: 2 }),
        "got: {error}"
    );
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

// ---------------------------------------------------------------------------------------------------------------
// Manifest::try_from
// ---------------------------------------------------------------------------------------------------------------

/// The `minos.toml` example in CLAUDE.md, section "Projects", copied verbatim.
const CLAUDE_EXAMPLE: &str = r#"[project]
name = "logparse"
kind = "optimize"            # tune | optimize | explore
program = "program.md"

[surface]
writable = ["src/parse.rs"]
readable = ["src/lib.rs"]

[source]
exclude = ["judge/", "docs/", "target/"]

[environment]
dockerfile = "minos.Dockerfile"
requires = { arch = "arm64" }

[commands]
build = ["cargo", "build", "--release"]
run = ["./target/release/bench", "--out", "/out/result.json"]
check = ["cargo", "check"]

[budget]
build = "2m"
run = "5m"
spend_usd = 40

[judge]
version = 1
command = ["./judge"]

[[judge.gate]]
metric = "identical_output"

[[judge.objective]]
metric = "throughput_mb_s"
goal = "maximize"
min_improvement = "5%"

[policy]
kind = "greedy"              # greedy | pareto | review
width = 4

[[proposer]]
kind = "single_shot"
model = "mid"
batch = 4
"#;

/// A valid integer sweep param over the example's writable file, appended after the last table.
const PARAM: &str = r#"
[[param]]
name = "chunk"
file = "src/parse.rs"
anchor = "const CHUNK: usize = "
min = 1
max = 64
step = 1
"#;

/// `text` with the first `from` replaced by `to`; an error if `from` is missing, so no test is vacuous.
fn edit(text: &str, from: &str, to: &str) -> Result<String, Box<dyn Error>> {
    if text.contains(from) {
        Ok(text.replacen(from, to, 1))
    } else {
        Err(format!("the base text does not contain {from:?}").into())
    }
}

/// The example with one edit.
fn example(from: &str, to: &str) -> Result<String, Box<dyn Error>> {
    edit(CLAUDE_EXAMPLE, from, to)
}

/// The example with `"judge/"` removed from `[source] exclude`, so the exclude rule cannot be what rejects a
/// judge-argument test, and then the judge command replaced.
fn example_with_judge_command(command: &str) -> Result<String, Box<dyn Error>> {
    let text = example(
        r#"exclude = ["judge/", "docs/", "target/"]"#,
        r#"exclude = ["docs/", "target/"]"#,
    )?;
    edit(
        &text,
        r#"command = ["./judge"]"#,
        &format!("command = {command}"),
    )
}

fn rejected(text: &str) -> Result<ManifestError, Box<dyn Error>> {
    match Manifest::parse_toml(text) {
        Ok(_) => Err("the manifest was accepted".into()),
        Err(error) => Ok(error),
    }
}

fn assert_field(text: &str, expected: &str) -> TestResult {
    let error = rejected(text)?;
    assert!(
        matches!(&error, ManifestError::Field(field) if field.field() == expected),
        "expected {expected} to be invalid, got: {error}"
    );
    Ok(())
}

fn assert_unknown_field(text: &str) -> TestResult {
    let error = rejected(text)?;
    assert!(matches!(error, ManifestError::Toml(_)), "got: {error}");
    let shown = error.to_string();
    assert!(shown.contains("unknown field"), "got: {shown}");
    assert!(shown.contains("line"), "got: {shown}");
    Ok(())
}

fn strings(paths: &std::collections::BTreeSet<RelPath>) -> Vec<&str> {
    paths.iter().map(RelPath::as_str).collect()
}

#[test]
fn claude_example_parses() -> TestResult {
    let manifest = Manifest::parse_toml(CLAUDE_EXAMPLE)?;
    assert_eq!(manifest.name().as_str(), "logparse");
    assert_eq!(manifest.kind(), ProjectKind::Optimize);
    assert_eq!(manifest.program(), &path("program.md")?);
    assert_eq!(strings(manifest.writable()), ["src/parse.rs"]);
    assert_eq!(strings(manifest.readable()), ["src/lib.rs"]);
    assert_eq!(strings(&manifest.surface()), ["src/lib.rs", "src/parse.rs"]);
    let excludes: Vec<String> = manifest
        .excludes()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(excludes, ["docs/", "judge/", "target/"]);
    assert_eq!(
        manifest.environment().dockerfile(),
        &path("minos.Dockerfile")?
    );
    assert_eq!(manifest.environment().arch(), Arch::Arm64);
    let commands = manifest.commands();
    assert_eq!(
        commands.build().as_slice(),
        argv(&["cargo", "build", "--release"]).as_slice()
    );
    assert_eq!(
        commands.run().program(),
        Some(&path("target/release/bench")?)
    );
    assert_eq!(
        commands.check().map(CommandArgv::as_slice),
        Some(argv(&["cargo", "check"]).as_slice())
    );
    let budget = manifest.budget();
    assert_eq!(budget.build_ms(), 120_000);
    assert_eq!(budget.run_ms(), 300_000);
    assert_eq!(budget.judge_ms(), DEFAULT_JUDGE_MS);
    assert_eq!(budget.spend_usd(), 40);
    assert_eq!(budget.spend_cap_micro(), 40_000_000);
    assert_eq!(budget.max_tokens(), TOKEN_CAP_DEFAULT);
    assert_eq!(budget.max_empty_generations(), EMPTY_GENERATIONS_DEFAULT);
    assert_eq!(budget.max_infra_failures(), INFRA_FAILURES_DEFAULT);
    let judge = manifest.judge();
    assert_eq!(judge.label(), 1);
    assert_eq!(judge.command().as_slice(), argv(&["./judge"]).as_slice());
    assert_eq!(judge.program(), Some(&path("judge")?));
    assert_eq!(strings(judge.paths()), ["judge"]);
    let gates: Vec<&str> = judge.gates().iter().map(MetricName::as_str).collect();
    assert_eq!(gates, ["identical_output"]);
    assert_eq!(judge.objectives().len(), 1);
    assert!(judge.held_out_metrics().is_empty());
    let declared: Vec<String> = judge
        .declared_metrics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(declared, ["identical_output", "throughput_mb_s"]);
    let objective = manifest.objective();
    assert_eq!(objective.metric().as_str(), "throughput_mb_s");
    assert_eq!(objective.goal(), Goal::Maximize);
    assert_eq!(objective.min_improvement().to_string(), "5%");
    assert_eq!(objective.held_out(), None);
    assert_eq!(manifest.policy().width(), 4);
    assert_eq!(manifest.policy().stall_step(), STALL_STEP_DEFAULT);
    let [proposer] = manifest.proposers() else {
        return Err("expected one proposer".into());
    };
    assert_eq!(proposer.tier(), Tier::SingleShot);
    assert_eq!(proposer.model().map(ModelId::as_str), Some("mid"));
    assert_eq!(proposer.batch(), 4);
    assert!(manifest.params().is_empty());
    Ok(())
}

#[test]
fn optional_fields_are_read_when_present() -> TestResult {
    let text = example(
        "spend_usd = 40\n",
        "spend_usd = 40\njudge = \"90s\"\nmax_tokens = 2000\nmax_empty_generations = 5\nmax_infra_failures = 6\n",
    )?;
    let text = edit(&text, "width = 4\n", "width = 4\nstall_step = 7\n")?;
    let text = format!("{text}{PARAM}");
    let manifest = Manifest::parse_toml(&text)?;
    assert_eq!(manifest.budget().judge_ms(), 90_000);
    assert_eq!(manifest.budget().max_tokens(), 2000);
    assert_eq!(manifest.budget().max_empty_generations(), 5);
    assert_eq!(manifest.budget().max_infra_failures(), 6);
    assert_eq!(manifest.policy().stall_step(), 7);
    let [param] = manifest.params() else {
        return Err("expected one param".into());
    };
    assert_eq!(param.name().as_str(), "chunk");
    assert!(matches!(param.domain(), ParamDomain::Int(range) if range.points() == 64));
    Ok(())
}

#[test]
fn absent_source_and_readable_are_empty() -> TestResult {
    let text = example("readable = [\"src/lib.rs\"]\n", "")?;
    let text = edit(
        &text,
        "[source]\nexclude = [\"judge/\", \"docs/\", \"target/\"]\n",
        "",
    )?;
    // With no excludes, the judge command may name no project path (rule 6), so it is a program on PATH.
    let text = edit(
        &text,
        r#"command = ["./judge"]"#,
        r#"command = ["judge-bin"]"#,
    )?;
    let manifest = Manifest::parse_toml(&text)?;
    assert!(manifest.readable().is_empty());
    assert!(manifest.excludes().is_empty());
    assert!(manifest.judge().paths().is_empty());
    Ok(())
}

#[test]
fn rejects_an_unknown_key_in_every_table() -> TestResult {
    for header in [
        "[project]\n",
        "[surface]\n",
        "[source]\n",
        "[environment]\n",
        "[commands]\n",
        "[budget]\n",
        "[judge]\n",
        "[[judge.gate]]\n",
        "[[judge.objective]]\n",
        "[policy]\n",
        "[[proposer]]\n",
    ] {
        assert_unknown_field(&example(header, &format!("{header}x = 1\n"))?)?;
    }
    assert_unknown_field(&example(
        r#"requires = { arch = "arm64" }"#,
        r#"requires = { arch = "arm64", x = 1 }"#,
    )?)?;
    assert_unknown_field(&format!("x = 1\n{CLAUDE_EXAMPLE}"))?;
    assert_unknown_field(&format!("{CLAUDE_EXAMPLE}{PARAM}x = 1\n"))?;
    Ok(())
}

#[test]
fn rejects_an_empty_writable() -> TestResult {
    assert_field(
        &example(r#"writable = ["src/parse.rs"]"#, "writable = []")?,
        "surface.writable",
    )
}

#[test]
fn rejects_a_duplicate_writable_entry() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "src/parse.rs"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::DuplicatePath { field: "surface.writable", path: found } if found.as_str() == "src/parse.rs"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_writable_overlapping_readable() -> TestResult {
    let text = example(
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "src/parse.rs"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::Overlap { path: found, with: "surface.readable" } if found.as_str() == "src/parse.rs"),
        "got: {error}"
    );
    // Deliberate-violation evidence: the exact message a user sees.
    assert_eq!(
        error.to_string(),
        "src/parse.rs is writable, so it cannot also be in surface.readable"
    );
    Ok(())
}

fn assert_overlap(text: &str, expected_path: &str, expected_with: &str) -> TestResult {
    let error = rejected(text)?;
    assert!(
        matches!(&error, ManifestError::Overlap { path, with } if path.as_str() == expected_path && *with == expected_with),
        "expected {expected_path} to overlap {expected_with}, got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_writable_program() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "program.md"]"#,
    )?;
    assert_overlap(&text, "program.md", "project.program")
}

#[test]
fn rejects_writable_dockerfile() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "minos.Dockerfile"]"#,
    )?;
    assert_overlap(&text, "minos.Dockerfile", "environment.dockerfile")
}

#[test]
fn rejects_writable_judge_program() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "judge"]"#,
    )?;
    assert_overlap(&text, "judge", "judge.command")
}

#[test]
fn rejects_reserved_surface_files() -> TestResult {
    for (field, from, to, path) in [
        (
            "surface.writable",
            r#"writable = ["src/parse.rs"]"#,
            r#"writable = ["src/parse.rs", "minos.toml"]"#,
            "minos.toml",
        ),
        (
            "surface.writable",
            r#"writable = ["src/parse.rs"]"#,
            r#"writable = ["src/parse.rs", "Minos.Lock"]"#,
            "Minos.Lock",
        ),
        (
            "surface.readable",
            r#"readable = ["src/lib.rs"]"#,
            r#"readable = ["src/lib.rs", "minos.lock"]"#,
            "minos.lock",
        ),
    ] {
        let error = rejected(&example(from, to)?)?;
        assert!(
            matches!(&error, ManifestError::Reserved { field: found_field, path: found } if *found_field == field && found.as_str() == path),
            "{to}: got: {error}"
        );
    }
    // The manifest may be readable: it is the user's own configuration, not a secret from the proposer.
    let text = example(
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "minos.toml"]"#,
    )?;
    assert!(Manifest::parse_toml(&text).is_ok());
    Ok(())
}

#[test]
fn rejects_readable_under_an_exclude() -> TestResult {
    let text = example(
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "docs/notes.md"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::Excluded { field: "surface.readable", path } if path.as_str() == "docs/notes.md"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_writable_under_an_exclude_in_another_case() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "Target/gen.rs"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::Excluded { field: "surface.writable", path } if path.as_str() == "Target/gen.rs"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_a_case_fold_collision() -> TestResult {
    let text = example(
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/Parse.rs"]"#,
    )?;
    let text = edit(
        &text,
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "src/parse.rs"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::CaseFoldCollision { first, second }
            if first.as_str() == "src/Parse.rs" && second.as_str() == "src/parse.rs"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_budgets_out_of_range() -> TestResult {
    assert_field(
        &example(r#"build = "2m""#, r#"build = "0s""#)?,
        "budget.build",
    )?;
    assert_field(&example(r#"run = "5m""#, r#"run = "61m""#)?, "budget.run")?;
    assert_field(
        &example("spend_usd = 40\n", "spend_usd = 40\njudge = \"0s\"\n")?,
        "budget.judge",
    )?;
    assert_field(
        &example("spend_usd = 40\n", "spend_usd = 40\nmax_tokens = 999\n")?,
        "budget.max_tokens",
    )
}

#[test]
fn rejects_spend_above_the_cap() -> TestResult {
    assert_field(
        &example("spend_usd = 40", "spend_usd = 10001")?,
        "budget.spend_usd",
    )?;
    assert!(Manifest::parse_toml(&example("spend_usd = 40", "spend_usd = 10000")?).is_ok());
    Ok(())
}

#[test]
fn rejects_spend_as_a_float() -> TestResult {
    let error = rejected(&example("spend_usd = 40", "spend_usd = 40.5")?)?;
    assert!(matches!(error, ManifestError::Toml(_)), "got: {error}");
    Ok(())
}

#[test]
fn rejects_greedy_with_two_objectives() -> TestResult {
    let text = example(
        "min_improvement = \"5%\"\n",
        "min_improvement = \"5%\"\n\n[[judge.objective]]\nmetric = \"latency_ms\"\ngoal = \"minimize\"\nmin_improvement = \"1%\"\n",
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(
            error,
            ManifestError::PolicyObjectiveMismatch { objectives: 2 }
        ),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_greedy_with_no_objective() -> TestResult {
    let text = example(
        "[[judge.objective]]\nmetric = \"throughput_mb_s\"\ngoal = \"maximize\"\nmin_improvement = \"5%\"\n",
        "",
    )?;
    let text = edit(
        &text,
        "command = [\"./judge\"]\n",
        "command = [\"./judge\"]\nobjective = []\n",
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(
            error,
            ManifestError::PolicyObjectiveMismatch { objectives: 0 }
        ),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_pareto_and_review_as_not_available() -> TestResult {
    for kind in ["pareto", "review"] {
        let text = example(r#"kind = "greedy""#, &format!("kind = \"{kind}\""))?;
        let error = rejected(&text)?;
        assert!(
            matches!(&error, ManifestError::PolicyNotAvailable { kind: found } if *found == kind),
            "got: {error}"
        );
    }
    Ok(())
}

#[test]
fn rejects_stall_step_out_of_range() -> TestResult {
    for value in [0, 101] {
        let text = example("width = 4\n", &format!("width = 4\nstall_step = {value}\n"))?;
        assert_field(&text, "policy.stall_step")?;
    }
    Ok(())
}

#[test]
fn rejects_breaker_limits_out_of_range() -> TestResult {
    for key in ["max_empty_generations", "max_infra_failures"] {
        for value in [0, 21] {
            let text = example(
                "spend_usd = 40\n",
                &format!("spend_usd = 40\n{key} = {value}\n"),
            )?;
            assert_field(&text, &format!("budget.{key}"))?;
        }
    }
    Ok(())
}

#[test]
fn rejects_width_out_of_range() -> TestResult {
    assert_field(&example("width = 4", "width = 0")?, "policy.width")?;
    assert_field(&example("width = 4", "width = 17")?, "policy.width")
}

#[test]
fn rejects_a_judge_label_of_zero() -> TestResult {
    assert_field(&example("version = 1", "version = 0")?, "judge.version")
}

#[test]
fn rejects_no_eligible_proposer() -> TestResult {
    let agentic = example(r#"kind = "single_shot""#, r#"kind = "agentic""#)?;
    assert!(matches!(rejected(&agentic)?, ManifestError::NoEligibleTier));
    let sweep = example(
        "kind = \"single_shot\"\nmodel = \"mid\"\n",
        "kind = \"sweep\"\n",
    )?;
    assert!(matches!(rejected(&sweep)?, ManifestError::NoEligibleTier));
    let manifest = Manifest::parse_toml(&format!("{sweep}{PARAM}"))?;
    assert!(matches!(
        manifest
            .proposers()
            .first()
            .map(|spec| spec.declared_eligibility(manifest.params())),
        Some(Eligibility::Eligible)
    ));
    Ok(())
}

#[test]
fn rejects_a_second_proposer_of_the_same_tier() -> TestResult {
    let text = format!(
        "{CLAUDE_EXAMPLE}\n[[proposer]]\nkind = \"single_shot\"\nmodel = \"big\"\nbatch = 1\n"
    );
    assert_field(&text, "proposer.kind")
}

#[test]
fn proposers_are_kept_in_tier_order() -> TestResult {
    let text = format!("{CLAUDE_EXAMPLE}\n[[proposer]]\nkind = \"sweep\"\nbatch = 2\n{PARAM}");
    let manifest = Manifest::parse_toml(&text)?;
    let tiers: Vec<Tier> = manifest
        .proposers()
        .iter()
        .map(ProposerSpec::tier)
        .collect();
    assert_eq!(tiers, [Tier::Sweep, Tier::SingleShot]);
    Ok(())
}

#[test]
fn rejects_a_param_whose_file_is_not_writable() -> TestResult {
    let param = edit(PARAM, r#"file = "src/parse.rs""#, r#"file = "src/lib.rs""#)?;
    let error = rejected(&format!("{CLAUDE_EXAMPLE}{param}"))?;
    assert!(
        matches!(&error, ManifestError::ParamFileNotWritable { name } if name.as_str() == "chunk"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_duplicate_param_names() -> TestResult {
    let error = rejected(&format!("{CLAUDE_EXAMPLE}{PARAM}{PARAM}"))?;
    assert!(
        matches!(&error, ManifestError::DuplicateParam { name } if name.as_str() == "chunk"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_a_param_with_both_or_neither_domain() -> TestResult {
    let both = edit(PARAM, "step = 1\n", "step = 1\nchoices = [\"1\"]\n")?;
    assert_field(&format!("{CLAUDE_EXAMPLE}{both}"), "param")?;
    let partial = edit(PARAM, "step = 1\n", "")?;
    assert_field(&format!("{CLAUDE_EXAMPLE}{partial}"), "param")?;
    let choices = edit(
        PARAM,
        "min = 1\nmax = 64\nstep = 1\n",
        "choices = [\"8\", \"16\"]\n",
    )?;
    assert!(Manifest::parse_toml(&format!("{CLAUDE_EXAMPLE}{choices}")).is_ok());
    Ok(())
}

#[test]
fn rejects_a_gate_named_like_an_objective() -> TestResult {
    let text = example(
        r#"metric = "identical_output""#,
        r#"metric = "throughput_mb_s""#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::DuplicateMetric { metric } if metric.as_str() == "throughput_mb_s"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_a_held_out_metric_named_like_a_gate() -> TestResult {
    let text = example(
        "min_improvement = \"5%\"\n",
        "min_improvement = \"5%\"\nheld_out = \"identical_output\"\n",
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::DuplicateMetric { metric } if metric.as_str() == "identical_output"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn a_held_out_pairing_is_recorded() -> TestResult {
    let text = example(
        "min_improvement = \"5%\"\n",
        "min_improvement = \"5%\"\nheld_out = \"throughput_held_out\"\n",
    )?;
    let manifest = Manifest::parse_toml(&text)?;
    assert_eq!(
        manifest.objective().held_out().map(MetricName::as_str),
        Some("throughput_held_out")
    );
    let held: Vec<String> = manifest
        .judge()
        .held_out_metrics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(held, ["throughput_held_out"]);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Judge argument paths
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn rejects_writable_judge_argument() -> TestResult {
    let text = example_with_judge_command(r#"["python3", "judge/score.py"]"#)?;
    let text = edit(
        &text,
        r#"writable = ["src/parse.rs"]"#,
        r#"writable = ["src/parse.rs", "judge/score.py"]"#,
    )?;
    assert_overlap(&text, "judge/score.py", "judge.command")
}

#[test]
fn rejects_writable_judge_option_value() -> TestResult {
    let text = example_with_judge_command(r#"["python3", "--data=src/parse.rs"]"#)?;
    assert_overlap(&text, "src/parse.rs", "judge.command")
}

#[test]
fn rejects_invalid_path_like_judge_argument() -> TestResult {
    let text = example_with_judge_command(r#"["python3", "judge/../src/parse.rs"]"#)?;
    let error = rejected(&text)?;
    assert!(
        matches!(error, ManifestError::JudgeArgPath { index: 1 }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn rejects_dot_and_slashed_flag_judge_arguments() -> TestResult {
    for (command, index) in [
        (r#"["python3", "score.py", "."]"#, 2),
        (r#"["python3", "./"]"#, 1),
        (r#"["python3", "-dsrc/parse.rs"]"#, 1),
        (r#"["python3", "judge\\score.py"]"#, 1),
    ] {
        let error = rejected(&example_with_judge_command(command)?)?;
        assert!(
            matches!(error, ManifestError::JudgeArgPath { index: found } if found == index),
            "{command}: got: {error}"
        );
    }
    Ok(())
}

#[test]
fn rejects_judge_argument_case_fold_collision() -> TestResult {
    let text = example_with_judge_command(r#"["python3", "src/Parse.rs"]"#)?;
    let error = rejected(&text)?;
    assert!(
        matches!(error, ManifestError::CaseFoldCollision { .. }),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn bare_judge_words_are_not_judge_paths() -> TestResult {
    // A program name, a plain argument, and a bare option value name no project file, so later phases that
    // mount or exclude judge paths never act on `python3` or `json`.
    let text = example(
        r#"command = ["./judge"]"#,
        r#"command = ["python3", "judge/score.py", "--format=json", "verbose", "-q"]"#,
    )?;
    let manifest = Manifest::parse_toml(&text)?;
    assert_eq!(strings(manifest.judge().paths()), ["judge/score.py"]);
    Ok(())
}

#[test]
fn rejects_a_judge_path_outside_the_source_excludes() -> TestResult {
    // The judge runs from its own pushed data; a judge path the snapshot would pack is refused, so the candidate
    // sandbox never sees the scorer's source. `example_with_judge_command` drops "judge/" from the excludes.
    for (command, path) in [
        (r#"["./judge"]"#, "judge"),
        (r#"["python3", "judge/score.py"]"#, "judge/score.py"),
    ] {
        let error = rejected(&example_with_judge_command(command)?)?;
        assert!(
            matches!(&error, ManifestError::JudgeNotExcluded { path: found } if found.as_str() == path),
            "{command}: got: {error}"
        );
    }
    // Covered by an exclude entry, in any ASCII case, it is accepted.
    let text = example(
        r#"exclude = ["judge/", "docs/", "target/"]"#,
        r#"exclude = ["Judge/", "docs/", "target/"]"#,
    )?;
    assert!(Manifest::parse_toml(&text).is_ok());
    Ok(())
}

#[test]
fn rejects_readable_judge_path() -> TestResult {
    let text = example(
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "judge"]"#,
    )?;
    let error = rejected(&text)?;
    assert!(
        matches!(&error, ManifestError::JudgeReadable { path } if path.as_str() == "judge"),
        "got: {error}"
    );
    Ok(())
}

#[test]
fn judge_flags_and_absolute_paths_are_not_judge_paths() -> TestResult {
    let text = example(
        r#"command = ["./judge"]"#,
        r#"command = ["./judge", "--out=/out/score.json", "-v"]"#,
    )?;
    let manifest = Manifest::parse_toml(&text)?;
    assert_eq!(strings(manifest.judge().paths()), ["judge"]);
    Ok(())
}

#[test]
fn serde_try_from_runs_the_same_rules() -> TestResult {
    let manifest: Manifest = toml::from_str(CLAUDE_EXAMPLE)?;
    assert_eq!(manifest.name().as_str(), "logparse");
    let overlap = example(
        r#"readable = ["src/lib.rs"]"#,
        r#"readable = ["src/lib.rs", "src/parse.rs"]"#,
    )?;
    let refused = toml::from_str::<Manifest>(&overlap)
        .err()
        .ok_or("serde accepted an overlapping manifest")?;
    assert!(
        refused.to_string().contains("src/parse.rs is writable"),
        "got: {refused}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// JudgeVersion
// ---------------------------------------------------------------------------------------------------------------

fn judge_version(text: &str) -> Result<JudgeVersion, Box<dyn Error>> {
    let manifest = Manifest::parse_toml(text)?;
    Ok(JudgeVersion::of(&TreeId::of(b"judge"), manifest.judge()))
}

#[test]
fn judge_version_golden_vector() -> TestResult {
    // DATA=$(printf 'judge' | shasum -a 256 | cut -d' ' -f1)   # 10e86c65...863e
    // { printf 'minos/judge/v1\0'; printf '%s' "$DATA" | xxd -r -p;
    //   printf '\0\0\0\0\0\0\0\1';                       # argc 1
    //   printf '\0\0\0\0\0\0\0\7./judge';                # argv[0]
    //   printf '\0\0\0\0\0\0\0\1';                       # 1 gate
    //   printf '\0\0\0\0\0\0\0\020identical_output';     # gate
    //   printf '\0\0\0\0\0\0\0\1';                       # 1 objective
    //   printf '\0\0\0\0\0\0\0\017throughput_mb_s';      # objective metric
    //   printf '\0\0\0\0\0\0\0\1';                       # maximize
    //   printf '\0\0\0\0\0\0\0\0'; } | shasum -a 256    # no held-out pairing; 149 input bytes
    assert_eq!(
        judge_version(CLAUDE_EXAMPLE)?.to_hex(),
        "3a2f24af3506ea83d0d0450c710b7813c5db892a487399d34a6d8cf894b0bc12"
    );
    Ok(())
}

#[test]
fn judge_version_changes_with_what_the_judge_measures() -> TestResult {
    let base = judge_version(CLAUDE_EXAMPLE)?;
    let changed = [
        example(
            r#"command = ["./judge"]"#,
            r#"command = ["./judge", "--strict"]"#,
        )?,
        example(
            r#"metric = "identical_output""#,
            r#"metric = "same_output""#,
        )?,
        example(
            r#"metric = "throughput_mb_s""#,
            r#"metric = "throughput_gb_s""#,
        )?,
        example(r#"goal = "maximize""#, r#"goal = "minimize""#)?,
        example(
            "min_improvement = \"5%\"\n",
            "min_improvement = \"5%\"\nheld_out = \"throughput_held_out\"\n",
        )?,
    ];
    for text in &changed {
        assert_ne!(judge_version(text)?, base);
    }
    let manifest = Manifest::parse_toml(CLAUDE_EXAMPLE)?;
    assert_ne!(
        JudgeVersion::of(&TreeId::of(b"judge2"), manifest.judge()),
        base
    );
    Ok(())
}

#[test]
fn judge_version_ignores_the_label_and_min_improvement() -> TestResult {
    let base = judge_version(CLAUDE_EXAMPLE)?;
    assert_eq!(
        judge_version(&example("version = 1", "version = 2")?)?,
        base
    );
    assert_eq!(
        judge_version(&example(
            r#"min_improvement = "5%""#,
            r#"min_improvement = "7%""#
        )?)?,
        base
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Environment memory (05-CONTEXT D-05, D-06)
// ---------------------------------------------------------------------------------------------------------------

/// The `[environment] requires` line of the example, after which the memory keys are inserted.
const REQUIRES: &str = "requires = { arch = \"arm64\" }\n";

/// The example with `lines` added to `[environment]`.
fn with_environment(lines: &str) -> Result<String, Box<dyn Error>> {
    example(REQUIRES, &format!("{REQUIRES}{lines}"))
}

/// The example with `memory_mib` and `scratch_mib` set; `None` leaves a key absent.
fn with_memory(memory: Option<u32>, scratch: Option<u32>) -> Result<String, Box<dyn Error>> {
    let lines: String = [
        memory.map(|memory| format!("memory_mib = {memory}\n")),
        scratch.map(|scratch| format!("scratch_mib = {scratch}\n")),
    ]
    .into_iter()
    .flatten()
    .collect();
    with_environment(&lines)
}

/// The three memory figures of a parsed manifest: memory, scratch, and the candidate allowance.
fn memory_of(text: &str) -> Result<(u16, u16, u16), Box<dyn Error>> {
    let manifest = Manifest::parse_toml(text)?;
    let environment = manifest.environment();
    Ok((
        environment.memory_mib(),
        environment.scratch_mib(),
        environment.candidate_allowance_mib(),
    ))
}

#[test]
fn limits_budget_holds_for_defaults() {
    assert_eq!(
        candidate_allowance_mib(DEFAULT_MEMORY_MIB, DEFAULT_SCRATCH_MIB),
        Some(768)
    );
    assert_eq!(candidate_allowance_mib(1279, 768), None);
    assert_eq!(candidate_allowance_mib(1280, 768), Some(256));
    // A scratch cap larger than the memory itself never underflows.
    assert_eq!(candidate_allowance_mib(128, 15_872), None);
}

#[test]
fn manifest_environment_memory_defaults() -> TestResult {
    assert_eq!(memory_of(CLAUDE_EXAMPLE)?, (2048, 1024, 768));
    assert_eq!(
        memory_of(CLAUDE_EXAMPLE)?,
        (DEFAULT_MEMORY_MIB, DEFAULT_SCRATCH_MIB, 768)
    );
    Ok(())
}

#[test]
fn manifest_environment_memory_explicit() -> TestResult {
    assert_eq!(
        memory_of(&with_memory(Some(4096), Some(2048))?)?,
        (4096, 2048, 1792)
    );
    Ok(())
}

#[test]
fn manifest_environment_memory_bounds() -> TestResult {
    assert_field(&with_memory(Some(127), None)?, "environment.memory_mib")?;
    assert_field(&with_memory(Some(16_385), None)?, "environment.memory_mib")?;
    assert_field(&with_memory(None, Some(63))?, "environment.scratch_mib")?;
    assert_field(
        &with_memory(Some(16_384), Some(15_873))?,
        "environment.scratch_mib",
    )?;
    // 128 MiB passes the leaf check; with the default scratch cap it fails only the budget rule.
    let error = rejected(&with_memory(Some(128), None)?)?;
    assert!(
        matches!(
            error,
            ManifestError::MemoryBudget {
                memory_mib: 128,
                scratch_mib: 1024
            }
        ),
        "got: {error}"
    );
    assert_eq!(
        memory_of(&with_memory(Some(16_384), None)?)?,
        (16_384, 1024, 15_104)
    );
    assert_eq!(memory_of(&with_memory(None, Some(64))?)?, (2048, 64, 1728));
    assert_eq!(
        memory_of(&with_memory(Some(16_384), Some(15_872))?)?,
        (16_384, 15_872, 256)
    );
    Ok(())
}

#[test]
fn manifest_environment_memory_out_of_u16_is_a_type_error() -> TestResult {
    let error = rejected(&with_memory(Some(70_000), None)?)?;
    assert!(matches!(error, ManifestError::Toml(_)), "got: {error}");
    Ok(())
}

#[test]
fn manifest_environment_memory_budget() -> TestResult {
    let error = rejected(&with_memory(Some(1279), Some(768))?)?;
    assert!(
        matches!(
            error,
            ManifestError::MemoryBudget {
                memory_mib: 1279,
                scratch_mib: 768
            }
        ),
        "got: {error}"
    );
    let shown = error.to_string();
    assert!(
        shown.contains(&GUEST_RESERVE_MIB.to_string()),
        "got: {shown}"
    );
    assert!(
        shown.contains(&MIN_CANDIDATE_ALLOWANCE_MIB.to_string()),
        "got: {shown}"
    );
    assert_eq!(
        memory_of(&with_memory(Some(1280), Some(768))?)?,
        (1280, 768, 256)
    );
    Ok(())
}

#[test]
fn manifest_environment_memory_unknown_key_is_refused() -> TestResult {
    assert_unknown_field(&with_environment("vcpus = 2\n")?)
}

#[test]
fn manifest_environment_memory_limits_name_the_bounds() -> TestResult {
    // The leaf messages are static text; this pins them to the constants so the two cannot drift.
    let cases = [
        (
            with_memory(Some(127), None)?,
            format!("{MIN_MEMORY_MIB} to {MAX_MEMORY_MIB} MiB"),
        ),
        (
            with_memory(None, Some(63))?,
            format!("{MIN_SCRATCH_MIB} to {MAX_SCRATCH_MIB} MiB"),
        ),
    ];
    for (text, expected) in cases {
        let error = rejected(&text)?;
        assert!(
            matches!(&error, ManifestError::Field(field) if field.limit() == expected),
            "expected {expected}, got: {error}"
        );
    }
    Ok(())
}
