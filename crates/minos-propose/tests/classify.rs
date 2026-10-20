//! The `claude -p` result classifier (06-CONTEXT D-12, D-13 amended, D-14, D-15; 06-RESEARCH Pattern 7): one test
//! per table row, from hand-written envelopes modelled on the real result lines of 02-SPIKE-EVIDENCE.md C-03 and
//! C-11.
//!
//! Every envelope carries fields the classifier does not declare (`session_id`, `num_turns`, `terminal_reason`,
//! `permission_denials`, and more), so each passing test also proves the envelope parse is lenient. No test makes
//! a model call.

use std::error::Error;

use minos_core::id::ModelId;
use minos_core::ledger::StopReason;
use minos_core::usage::Usage;
use minos_propose::claude_cli::classify::{Exit, classify};
use minos_propose::provider::{
    Completion, EmptyCause, InfraCause, ModelResponse, ProviderError, ProviderStop,
};

type TestResult = Result<(), Box<dyn Error>>;

/// Fields every envelope carries that the classifier never declares, as in the C-03 line.
const EXTRA: &str = r#""duration_api_ms":867,"session_id":"05be13f6-1dc8-46c8-aaeb-0819762d8476","num_turns":2,"terminal_reason":"completed","permission_denials":[],"fast_mode_state":"off","fast_mode_disabled_reason":"sdk_opt_in_required","subagent_stats":{"spawned":0,"by_type":{}},"safety_stops":0,"ttft_ms":877,"duration_ms":911,"uuid":"<redacted>","queued_turn_count":0,"result_index":0"#;

/// The usage fields of a C-03-like line, with numeric dollar estimates.
const USAGE: &str = r#""total_cost_usd":0.0123,"usage":{"input_tokens":2,"cache_creation_input_tokens":3621,"cache_read_input_tokens":0,"output_tokens":428,"output_tokens_details":{"thinking_tokens":0},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":3621,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":428,"cache_read_input_tokens":0,"cache_creation_input_tokens":3621,"type":"message"}],"speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":428,"cacheReadInputTokens":0,"cacheCreationInputTokens":3621,"webSearchRequests":0,"costUSD":0.0123,"contextWindow":1000000,"maxOutputTokens":128000,"thinkingTokens":0,"canonicalModel":"claude-haiku-5-5","provider":"firstParty","costBasis":"list"}}"#;

/// The same usage fields with string-typed dollar estimates, which no declared field could bind.
const USAGE_STRING_COST: &str = r#""total_cost_usd":"not a number","usage":{"input_tokens":2,"cache_creation_input_tokens":3621,"cache_read_input_tokens":0,"output_tokens":428,"output_tokens_details":{"thinking_tokens":0},"service_tier":"standard","speed":"standard","fallback_credit":null},"modelUsage":{"claude-haiku-5-5":{"inputTokens":2,"outputTokens":428,"cacheReadInputTokens":0,"cacheCreationInputTokens":3621,"costUSD":"not a number","provider":"firstParty"}}"#;

const STRUCTURED: &str = r#""type":"result","subtype":"success","is_error":false,"api_error_status":null,"stop_reason":"tool_use","result":"{\"proposals\":[]}","structured_output":{"proposals":[{"rationale":"Raise the value.","files":[{"path":"src/k.txt","content":"value = 2\n"}]}]}"#;

const MAX_TURNS: &str = r#""type":"result","subtype":"error_max_turns","is_error":true,"stop_reason":"tool_use","errors":["Reached maximum number of turns (1)"]"#;

const LIMIT_429: &str = r#""type":"result","subtype":"success","is_error":true,"api_error_status":429,"stop_reason":"stop_sequence","result":"You've hit your session limit · resets 3:45pm""#;

fn with_usage(core: &str) -> String {
    format!("{{{EXTRA},{USAGE},{core}}}")
}

fn with_string_cost(core: &str) -> String {
    format!("{{{EXTRA},{USAGE_STRING_COST},{core}}}")
}

fn without_usage(core: &str) -> String {
    format!("{{{EXTRA},{core}}}")
}

fn usage() -> Usage {
    Usage::tokens(2, 428, 3621, 0)
}

fn ok(result: Result<ModelResponse, ProviderError>) -> Result<ModelResponse, Box<dyn Error>> {
    result.map_err(|error| format!("expected a response, got {error:?}").into())
}

fn err(result: Result<ModelResponse, ProviderError>) -> Result<ProviderError, Box<dyn Error>> {
    result
        .err()
        .ok_or_else(|| Box::<dyn Error>::from("expected an error, got a response"))
}

fn stop(stop: ProviderStop, usage: Option<Usage>) -> ProviderError {
    ProviderError::Stop { stop, usage }
}

fn infra(cause: InfraCause, usage: Option<Usage>) -> ProviderError {
    ProviderError::Infra { cause, usage }
}

fn assert_empty(response: &ModelResponse, cause: EmptyCause) {
    assert!(
        matches!(response.completion(), Completion::Empty(found) if *found == cause),
        "expected Empty({cause:?})"
    );
}

fn success_error(status: &str, result: &str) -> String {
    with_usage(&format!(
        r#""type":"result","subtype":"success","is_error":true,"api_error_status":{status},"stop_reason":"stop_sequence","result":{}"#,
        serde_json::to_string(result).unwrap_or_default()
    ))
}

fn during_execution(error: &str) -> String {
    with_usage(&format!(
        r#""type":"result","subtype":"error_during_execution","is_error":true,"errors":[{}]"#,
        serde_json::to_string(error).unwrap_or_default()
    ))
}

#[test]
fn success_with_structured_output_is_a_completion() -> TestResult {
    let response = ok(classify(with_usage(STRUCTURED).as_bytes(), Exit::Code(0)))?;
    assert!(matches!(response.completion(), Completion::Structured(_)));
    assert_eq!(response.usage(), &usage());
    let haiku: ModelId = "claude-haiku-5-5".parse()?;
    assert_eq!(response.models().get(&haiku), Some(&usage()));
    assert_eq!(response.models().len(), 1);
    assert_eq!(response.resolved_model(), Some(&haiku));
    Ok(())
}

#[test]
fn success_without_structured_output_is_infra() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"success","is_error":false,"api_error_status":null,"stop_reason":"end_turn","result":"7""#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(
        error,
        infra(InfraCause::MissingStructuredOutput, Some(usage()))
    );
    Ok(())
}

#[test]
fn success_with_a_non_zero_exit_is_a_mismatch() -> TestResult {
    let error = err(classify(with_usage(STRUCTURED).as_bytes(), Exit::Code(1)))?;
    assert_eq!(
        error,
        infra(InfraCause::ExitMismatch { code: Some(1) }, Some(usage()))
    );
    Ok(())
}

#[test]
fn success_without_usage_is_missing_usage() -> TestResult {
    let error = err(classify(
        without_usage(STRUCTURED).as_bytes(),
        Exit::Code(0),
    ))?;
    assert_eq!(error, infra(InfraCause::MissingUsage, None));
    Ok(())
}

#[test]
fn success_without_is_error_is_invalid() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"success","api_error_status":null,"structured_output":{"proposals":[]}"#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::EnvelopeInvalid, Some(usage())));
    Ok(())
}

#[test]
fn status_429_is_a_limit_stop_with_usage() -> TestResult {
    let line = with_usage(LIMIT_429);
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn limit_text_with_a_null_status_is_a_limit_stop() -> TestResult {
    let line = success_error("null", "You've hit your weekly limit · resets Oct 21");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn limit_text_with_a_typographic_apostrophe_is_a_limit_stop() -> TestResult {
    let line = success_error(
        "null",
        "You\u{2019}ve hit your session limit · resets 3:45pm",
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn reached_your_limit_text_is_a_limit_stop() -> TestResult {
    let line = success_error("null", "You've reached your Opus limit");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn out_of_usage_credits_is_a_limit_stop() -> TestResult {
    let line = success_error("null", "You're out of usage credits");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn status_401_is_an_auth_stop() -> TestResult {
    let line = success_error(
        "401",
        "Failed to authenticate. API Error: 401 {\"type\":\"error\"}",
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn status_403_is_an_auth_stop() -> TestResult {
    let line = success_error("403", "API Error: 403 forbidden");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn oauth_revoked_with_a_null_status_is_an_auth_stop() -> TestResult {
    let line = success_error("null", "OAuth token revoked · Please run /login");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn not_logged_in_with_a_null_status_is_an_auth_stop() -> TestResult {
    let line = success_error("null", "Not logged in · Please run /login");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn a_stop_without_usage_carries_none() -> TestResult {
    let error = err(classify(without_usage(LIMIT_429).as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, None));
    Ok(())
}

#[test]
fn a_refusal_is_an_empty_call_with_usage() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"success","is_error":true,"api_error_status":null,"stop_reason":"refusal","result":"I can't help with that.""#,
    );
    let response = ok(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_empty(&response, EmptyCause::Refusal);
    assert_eq!(response.usage(), &usage());
    Ok(())
}

#[test]
fn a_refusal_without_usage_is_missing_usage() -> TestResult {
    let line = without_usage(
        r#""type":"result","subtype":"success","is_error":true,"api_error_status":null,"stop_reason":"refusal","result":"No.""#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, infra(InfraCause::MissingUsage, None));
    Ok(())
}

#[test]
fn status_529_is_an_api_error() -> TestResult {
    let line = success_error("529", "API Error: 529 Overloaded");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(
        error,
        infra(InfraCause::ApiError { status: Some(529) }, Some(usage()))
    );
    Ok(())
}

#[test]
fn status_500_is_an_api_error() -> TestResult {
    let line = success_error("500", "API Error: 500 Internal server error");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(
        error,
        infra(InfraCause::ApiError { status: Some(500) }, Some(usage()))
    );
    Ok(())
}

#[test]
fn an_output_token_maximum_is_an_api_error() -> TestResult {
    let line = success_error(
        "null",
        "API Error: Claude's response exceeded the 32000 output token maximum.",
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(
        error,
        infra(InfraCause::ApiError { status: None }, Some(usage()))
    );
    Ok(())
}

#[test]
fn an_out_of_range_status_is_an_unknown_status() -> TestResult {
    let line = success_error("70000", "API Error");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(
        error,
        infra(InfraCause::ApiError { status: None }, Some(usage()))
    );
    Ok(())
}

#[test]
fn max_turns_is_an_empty_call_with_usage() -> TestResult {
    let response = ok(classify(with_usage(MAX_TURNS).as_bytes(), Exit::Code(1)))?;
    assert_empty(&response, EmptyCause::MaxTurns);
    assert_eq!(response.usage(), &usage());
    Ok(())
}

#[test]
fn max_turns_without_usage_is_missing_usage() -> TestResult {
    let error = err(classify(without_usage(MAX_TURNS).as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, infra(InfraCause::MissingUsage, None));
    Ok(())
}

#[test]
fn structured_output_retries_is_an_empty_call() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"error_max_structured_output_retries","is_error":true,"errors":["Failed to provide valid structured output after 5 attempts"]"#,
    );
    let response = ok(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_empty(&response, EmptyCause::StructuredOutputRetries);
    assert_eq!(response.usage(), &usage());
    Ok(())
}

#[test]
fn an_execution_error_with_a_limit_message_is_a_limit_stop() -> TestResult {
    let line = during_execution("You've hit your weekly limit · resets Oct 21");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn an_execution_error_with_an_auth_message_is_an_auth_stop() -> TestResult {
    let line = during_execution("Login expired · Please run /login");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn an_execution_error_checks_every_entry() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"error_during_execution","is_error":true,"errors":["tool failed","OAuth token has expired"]"#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, stop(ProviderStop::Auth, Some(usage())));
    Ok(())
}

#[test]
fn any_other_execution_error_is_infra() -> TestResult {
    let line = during_execution("TypeError: undefined is not a function");
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, infra(InfraCause::ExecutionError, Some(usage())));
    Ok(())
}

#[test]
fn the_budget_subtype_is_infra() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"error_max_budget_usd","is_error":true,"errors":["Reached maximum budget"]"#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, infra(InfraCause::BudgetExceeded, Some(usage())));
    Ok(())
}

#[test]
fn an_unknown_subtype_is_infra() -> TestResult {
    let line = with_usage(r#""type":"result","subtype":"error_new_kind","is_error":true"#);
    let error = err(classify(line.as_bytes(), Exit::Code(1)))?;
    assert_eq!(error, infra(InfraCause::UnknownSubtype, Some(usage())));
    Ok(())
}

#[test]
fn a_missing_subtype_is_unknown() -> TestResult {
    let line = with_usage(r#""type":"result","is_error":false"#);
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::UnknownSubtype, Some(usage())));
    Ok(())
}

#[test]
fn a_message_other_than_a_result_is_infra() -> TestResult {
    let line = with_usage(r#""type":"assistant","subtype":"success","is_error":false"#);
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::NotAResult, Some(usage())));
    Ok(())
}

#[test]
fn non_json_is_an_invalid_envelope_without_usage() -> TestResult {
    for stdout in [
        b"Error: something went wrong\n".as_slice(),
        b"",
        b"[1,2,3]",
        b"\xff\xfe",
    ] {
        let error = err(classify(stdout, Exit::Code(1)))?;
        assert_eq!(error, infra(InfraCause::EnvelopeInvalid, None));
    }
    Ok(())
}

#[test]
fn trailing_garbage_is_an_invalid_envelope() -> TestResult {
    let mut line = with_usage(STRUCTURED);
    line.push_str("\ngarbage");
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::EnvelopeInvalid, None));

    let twice = format!("{0}\n{0}", with_usage(STRUCTURED));
    let error = err(classify(twice.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::EnvelopeInvalid, None));

    let mut newline = with_usage(STRUCTURED);
    newline.push('\n');
    ok(classify(newline.as_bytes(), Exit::Code(0)))?;
    Ok(())
}

#[test]
fn a_mistyped_declared_field_is_an_invalid_envelope() -> TestResult {
    let line = with_usage(
        r#""type":"result","subtype":"success","is_error":"no","structured_output":{"proposals":[]}"#,
    );
    let error = err(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(error, infra(InfraCause::EnvelopeInvalid, None));
    Ok(())
}

#[test]
fn a_signal_is_infra_whatever_stdout_holds() -> TestResult {
    let error = err(classify(with_usage(STRUCTURED).as_bytes(), Exit::Signal(9)))?;
    assert_eq!(error, infra(InfraCause::Signal { signal: 9 }, None));
    Ok(())
}

#[test]
fn string_dollar_fields_classify_like_their_numeric_twins() -> TestResult {
    let numeric = ok(classify(with_usage(STRUCTURED).as_bytes(), Exit::Code(0)))?;
    let string = ok(classify(
        with_string_cost(STRUCTURED).as_bytes(),
        Exit::Code(0),
    ))?;
    assert!(matches!(string.completion(), Completion::Structured(_)));
    assert_eq!(string.usage(), numeric.usage());
    assert_eq!(string.models(), numeric.models());

    let response = ok(classify(
        with_string_cost(MAX_TURNS).as_bytes(),
        Exit::Code(1),
    ))?;
    assert_empty(&response, EmptyCause::MaxTurns);
    assert_eq!(response.usage(), &usage());

    let error = err(classify(
        with_string_cost(LIMIT_429).as_bytes(),
        Exit::Code(1),
    ))?;
    assert_eq!(error, stop(ProviderStop::Limit, Some(usage())));
    Ok(())
}

#[test]
fn model_usage_keys_that_are_not_model_ids_are_skipped() -> TestResult {
    let line = format!(
        "{{{EXTRA},{core},{usage}}}",
        core = STRUCTURED,
        usage = r#""usage":{"input_tokens":5,"output_tokens":6,"cache_creation_input_tokens":7,"cache_read_input_tokens":8},"modelUsage":{"claude-haiku-5-5":{"inputTokens":1,"outputTokens":2,"cacheReadInputTokens":3,"cacheCreationInputTokens":4,"costUSD":1.5},"-bad key":{"inputTokens":9},"claude-sonnet-5":{"outputTokens":10}}"#
    );
    let response = ok(classify(line.as_bytes(), Exit::Code(0)))?;
    assert_eq!(response.usage(), &Usage::tokens(5, 6, 7, 8));
    let haiku: ModelId = "claude-haiku-5-5".parse()?;
    let sonnet: ModelId = "claude-sonnet-5".parse()?;
    assert_eq!(response.models().len(), 2);
    assert_eq!(
        response.models().get(&haiku),
        Some(&Usage::tokens(1, 2, 4, 3))
    );
    assert_eq!(
        response.models().get(&sonnet),
        Some(&Usage::tokens(0, 10, 0, 0))
    );
    assert_eq!(response.resolved_model(), None);
    Ok(())
}

#[test]
fn provider_stops_map_to_ledger_stop_reasons() {
    assert_eq!(ProviderStop::Limit.stop_reason(), StopReason::ProviderLimit);
    assert_eq!(ProviderStop::Auth.stop_reason(), StopReason::ProviderAuth);
    assert_eq!(
        stop(ProviderStop::Auth, None).stop_reason(),
        Some(StopReason::ProviderAuth)
    );
    assert_eq!(
        stop(ProviderStop::Limit, Some(usage())).stop_reason(),
        Some(StopReason::ProviderLimit)
    );
    assert_eq!(infra(InfraCause::Spawn, None).stop_reason(), None);
}
