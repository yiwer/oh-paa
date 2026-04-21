use pa_core::AppError;
use pa_orchestrator::{
    AnalysisBarState, Executor, FixtureLlmClient, PromptRegistry, PromptResultSemantics,
    PromptSpec, RetryPolicyClass,
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
    let registry = PromptRegistry::default().with_spec(make_spec(serde_json::json!({
        "type": "object",
        "required": ["bullish_case", "bearish_case"],
        "properties": {
            "bullish_case": { "type": "object" },
            "bearish_case": { "type": "object" }
        }
    })));
    let executor = Executor::new(
        registry,
        FixtureLlmClient::with_json(serde_json::json!({"bullish_case": {}})),
    );

    let err = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap_err();

    match err {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("schema validation failed"));
        }
        other => panic!("expected analysis error, got: {other}"),
    }
}

#[tokio::test]
async fn executor_returns_valid_structured_output() {
    let registry = PromptRegistry::default().with_spec(make_spec(serde_json::json!({
        "type": "object",
        "required": ["bullish_case", "bearish_case"],
        "properties": {
            "bullish_case": { "type": "object" },
            "bearish_case": { "type": "object" }
        }
    })));
    let expected = serde_json::json!({
        "bullish_case": {"entry": "breakout"},
        "bearish_case": {"entry": "pullback"}
    });
    let executor = Executor::new(registry, FixtureLlmClient::with_json(expected.clone()));

    let output = executor
        .execute_json("shared_bar_analysis", "v1", &serde_json::json!({"foo": "bar"}))
        .await
        .unwrap();

    assert_eq!(output, expected);
}
