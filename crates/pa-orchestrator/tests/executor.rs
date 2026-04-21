use pa_core::AppError;
use pa_orchestrator::{
    AnalysisBarState, ExecutionOutcome, Executor, FixtureLlmClient, PromptRegistry,
    PromptResultSemantics, PromptSpec, RetryPolicyClass,
};

fn make_spec(output_json_schema: serde_json::Value) -> PromptSpec {
    PromptSpec {
        prompt_key: "shared_bar_analysis".to_string(),
        prompt_version: "v1".to_string(),
        task_type: "shared_bar_analysis".to_string(),
        system_prompt: "Return JSON only".to_string(),
        input_schema_version: "v1".to_string(),
        output_schema_version: "v1".to_string(),
        output_json_schema,
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed],
    }
}

#[tokio::test]
async fn executor_fails_when_prompt_spec_is_missing() {
    let registry = PromptRegistry::default();
    let executor = Executor::new(registry, FixtureLlmClient::with_json(serde_json::json!({})));

    let err = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap_err();

    match err {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("missing prompt spec"));
        }
        other => panic!("expected analysis error, got: {other}"),
    }
}

#[tokio::test]
async fn executor_fails_when_output_does_not_match_schema() {
    let registry = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({
            "type": "object",
            "required": ["bullish_case", "bearish_case"],
            "properties": {
                "bullish_case": { "type": "object" },
                "bearish_case": { "type": "object" }
            }
        })))
        .unwrap();
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_json(serde_json::json!({"bullish_case": {}})),
    );

    let outcome = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap();

    match outcome {
        ExecutionOutcome::SchemaValidationFailed(attempt) => {
            assert_eq!(attempt.llm_provider, "fixture");
            assert_eq!(attempt.model, "fixture-json");
            assert_eq!(
                attempt.request_payload_json,
                serde_json::json!({
                    "system_prompt": "Return JSON only",
                    "input_json": {"foo": "bar"}
                })
            );
            assert_eq!(
                attempt.raw_response_json,
                Some(serde_json::json!({"bullish_case": {}}))
            );
            assert_eq!(
                attempt.parsed_output_json,
                Some(serde_json::json!({"bullish_case": {}}))
            );
            assert!(
                attempt
                    .schema_validation_error
                    .as_deref()
                    .is_some_and(|message| message.contains("bearish_case"))
            );
            assert_eq!(attempt.outbound_error_message, None);
        }
        other => panic!("expected schema validation failure, got: {other:?}"),
    }
}

#[tokio::test]
async fn executor_returns_valid_structured_output() {
    let registry = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({
            "type": "object",
            "required": ["bullish_case", "bearish_case"],
            "properties": {
                "bullish_case": { "type": "object" },
                "bearish_case": { "type": "object" }
            }
        })))
        .unwrap();
    let expected = serde_json::json!({
        "bullish_case": {"entry": "breakout"},
        "bearish_case": {"entry": "pullback"}
    });
    let executor = Executor::new(registry, FixtureLlmClient::with_json(expected.clone()));

    let output = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap();

    match output {
        ExecutionOutcome::Success(attempt) => {
            assert_eq!(attempt.llm_provider, "fixture");
            assert_eq!(attempt.model, "fixture-json");
            assert_eq!(
                attempt.request_payload_json,
                serde_json::json!({
                    "system_prompt": "Return JSON only",
                    "input_json": {"foo": "bar"}
                })
            );
            assert_eq!(attempt.raw_response_json, Some(expected.clone()));
            assert_eq!(attempt.parsed_output_json, Some(expected));
            assert_eq!(attempt.schema_validation_error, None);
            assert_eq!(attempt.outbound_error_message, None);
        }
        other => panic!("expected success output, got: {other:?}"),
    }
}

#[tokio::test]
async fn executor_returns_outbound_failure_with_attempt_context() {
    let registry = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({
            "type": "object"
        })))
        .unwrap();
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_provider_error("upstream timeout"),
    );

    let outcome = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap();

    match outcome {
        ExecutionOutcome::OutboundCallFailed { attempt, error } => {
            assert_eq!(attempt.llm_provider, "fixture");
            assert_eq!(attempt.model, "fixture-json");
            assert_eq!(
                attempt.request_payload_json,
                serde_json::json!({
                    "system_prompt": "Return JSON only",
                    "input_json": {"foo": "bar"}
                })
            );
            assert_eq!(attempt.raw_response_json, None);
            assert_eq!(attempt.parsed_output_json, None);
            assert_eq!(attempt.schema_validation_error, None);
            assert!(
                attempt
                    .outbound_error_message
                    .as_deref()
                    .is_some_and(|message| message.contains("provider error"))
            );

            match error {
                AppError::Provider { message, .. } => {
                    assert!(message.contains("upstream timeout"));
                }
                other => panic!("expected provider error, got: {other}"),
            }
        }
        other => panic!("expected outbound failure output, got: {other:?}"),
    }
}

#[test]
fn prompt_registry_rejects_duplicate_prompt_versions() {
    let registry = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({"type": "object"})))
        .unwrap();
    let err = registry
        .with_spec(make_spec(serde_json::json!({"type": "object"})))
        .unwrap_err();

    match err {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("duplicate prompt spec"));
        }
        other => panic!("expected analysis error, got: {other}"),
    }
}

#[test]
fn prompt_registry_rejects_invalid_output_json_schema() {
    let err = PromptRegistry::default()
        .with_spec(make_spec(serde_json::json!({
            "type": 7
        })))
        .unwrap_err();

    match err {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("invalid output schema"));
        }
        other => panic!("expected analysis error, got: {other}"),
    }
}
