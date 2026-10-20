//! The provider abstraction's value types: `ModelResponse::resolved_model`, `Prompt`, and the compile-time proof
//! that hostile `ModelOutput` has no display or copy path.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::ops::Deref;

use minos_core::id::{ModelId, PromptHash};
use minos_core::usage::Usage;
use minos_propose::limits::MAX_PROMPT_BYTES;
use minos_propose::prompt::{Prompt, PromptError};
use minos_propose::provider::{
    Completion, EmptyCause, InfraCause, ModelOutput, ModelResponse, ProviderError, ProviderStop,
};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------------------------------------------
// ModelOutput implements none of Debug, Display, Clone, Deref
//
// The trick (as in the `static_assertions` crate's `assert_not_impl_any!`): `NotImplemented<A>` has a default
// associated function. One blanket impl covers every type at the unit parameter `()`. Each forbidden trait gets
// one more blanket impl at its own marker parameter, applying only to types that implement that trait. Naming
// the function through `ModelOutput` with the parameter left to inference then compiles only while exactly one
// impl applies, the unit one: if `ModelOutput` ever implemented Debug, Display, Clone, or Deref, a second impl
// would apply, inference would be ambiguous, and this file would fail to build.
// ---------------------------------------------------------------------------------------------------------------

trait NotImplemented<A> {
    fn check() {}
}

impl<T: ?Sized> NotImplemented<()> for T {}

struct ViaDebug;
impl<T: ?Sized + Debug> NotImplemented<ViaDebug> for T {}

struct ViaDisplay;
impl<T: ?Sized + Display> NotImplemented<ViaDisplay> for T {}

struct ViaClone;
impl<T: Clone> NotImplemented<ViaClone> for T {}

struct ViaDeref;
impl<T: ?Sized + Deref> NotImplemented<ViaDeref> for T {}

#[test]
fn model_output_has_no_debug_display_clone_or_deref() {
    <ModelOutput as NotImplemented<_>>::check();
}

// ---------------------------------------------------------------------------------------------------------------
// ModelResponse
// ---------------------------------------------------------------------------------------------------------------

fn model(name: &str) -> Result<ModelId, Box<dyn Error>> {
    Ok(name.parse::<ModelId>()?)
}

fn response(models: BTreeMap<ModelId, Usage>) -> ModelResponse {
    ModelResponse::new(
        Completion::Structured(ModelOutput::from_value(serde_json::Value::Null)),
        Usage::tokens(1, 2, 3, 4),
        models,
    )
}

#[test]
fn resolved_model_is_the_single_entry() -> TestResult {
    let mut models = BTreeMap::new();
    models.insert(model("claude-haiku-5-5")?, Usage::tokens(1, 2, 3, 4));
    let response = response(models);
    assert_eq!(response.resolved_model(), Some(&model("claude-haiku-5-5")?));
    assert_eq!(response.usage(), &Usage::tokens(1, 2, 3, 4));
    assert_eq!(response.models().len(), 1);
    Ok(())
}

#[test]
fn resolved_model_is_none_without_entries() {
    assert_eq!(response(BTreeMap::new()).resolved_model(), None);
}

#[test]
fn resolved_model_is_none_with_several_entries() -> TestResult {
    let mut models = BTreeMap::new();
    models.insert(model("claude-haiku-5-5")?, Usage::tokens(1, 0, 0, 0));
    models.insert(model("claude-sonnet-5")?, Usage::tokens(0, 1, 0, 0));
    assert_eq!(response(models).resolved_model(), None);
    Ok(())
}

#[test]
fn empty_completion_keeps_its_cause() {
    let response = ModelResponse::new(
        Completion::Empty(EmptyCause::MaxTurns),
        Usage::default(),
        BTreeMap::new(),
    );
    assert!(matches!(
        response.completion(),
        Completion::Empty(EmptyCause::MaxTurns)
    ));
    assert!(matches!(
        response.into_completion(),
        Completion::Empty(EmptyCause::MaxTurns)
    ));
}

// ---------------------------------------------------------------------------------------------------------------
// ProviderError
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn provider_error_separates_stops_from_infrastructure() {
    let stop = ProviderError::Stop {
        stop: ProviderStop::Limit,
        usage: Some(Usage::tokens(5, 0, 0, 0)),
    };
    let infra = ProviderError::Infra {
        cause: InfraCause::Deadline,
        usage: None,
    };
    assert!(!stop.is_infra());
    assert!(infra.is_infra());
    assert_eq!(stop.usage(), Some(&Usage::tokens(5, 0, 0, 0)));
    assert_eq!(infra.usage(), None);
    assert_eq!(
        stop.to_string(),
        "the model provider refused for a rate or usage limit"
    );
    assert_eq!(
        ProviderStop::Auth.to_string(),
        "the model provider rejected the login"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn prompt_keeps_prefix_then_suffix() -> TestResult {
    let prompt = Prompt::new("stable prefix\n", "dynamic suffix\n")?;
    assert_eq!(prompt.as_str(), "stable prefix\ndynamic suffix\n");
    assert_eq!(prompt.as_bytes(), b"stable prefix\ndynamic suffix\n");
    assert_eq!(prompt.prefix_len(), 14);
    assert_eq!(prompt.len(), 29);
    assert_eq!(prompt.prefix(), "stable prefix\n");
    assert_eq!(prompt.suffix(), "dynamic suffix\n");
    assert_eq!(
        prompt.hash(),
        &PromptHash::of(b"stable prefix\ndynamic suffix\n")
    );
    Ok(())
}

#[test]
fn prompt_hash_covers_the_whole_text() -> TestResult {
    let one = Prompt::new("ab", "c")?;
    let two = Prompt::new("a", "bc")?;
    assert_eq!(one.as_str(), two.as_str());
    assert_eq!(one.hash(), two.hash());
    assert_ne!(one.prefix_len(), two.prefix_len());
    Ok(())
}

#[test]
fn prompt_debug_prints_no_text() -> TestResult {
    let prompt = Prompt::new("secret-looking prefix", "secret-looking suffix")?;
    let shown = format!("{prompt:?}");
    assert!(!shown.contains("secret-looking"));
    assert!(shown.contains("42"));
    Ok(())
}

#[test]
fn prompt_at_the_cap_is_accepted() -> TestResult {
    let prefix = "a".repeat(MAX_PROMPT_BYTES - 1);
    let prompt = Prompt::new(&prefix, "b")?;
    assert_eq!(prompt.len(), MAX_PROMPT_BYTES);
    Ok(())
}

#[test]
fn prompt_over_the_cap_is_refused() -> TestResult {
    let prefix = "a".repeat(MAX_PROMPT_BYTES);
    match Prompt::new(&prefix, "b") {
        Err(PromptError::TooLarge { bytes, limit }) => {
            assert_eq!(bytes, MAX_PROMPT_BYTES + 1);
            assert_eq!(limit, MAX_PROMPT_BYTES);
            Ok(())
        }
        Err(other) => Err(format!("wrong error: {other}").into()),
        Ok(_) => Err("an over-cap prompt was accepted".into()),
    }
}
