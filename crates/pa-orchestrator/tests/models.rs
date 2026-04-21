use pa_orchestrator::{
    AnalysisBarState, AnalysisTaskStatus, PromptResultSemantics, PromptSpec, RetryPolicyClass,
};
use std::io::ErrorKind;

#[test]
fn prompt_spec_exposes_phase2_contract_fields() {
    let spec = PromptSpec {
        prompt_key: "shared_bar_analysis".into(),
        prompt_version: "v1".into(),
        task_type: "shared_bar_analysis".into(),
        system_prompt: "Return JSON only".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({"type": "object"}),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
    };

    assert_eq!(AnalysisTaskStatus::Pending.as_str(), "pending");
    assert_eq!(AnalysisTaskStatus::RetryWaiting.as_str(), "retry_waiting");
    assert_eq!(
        AnalysisTaskStatus::from_db("pending"),
        Some(AnalysisTaskStatus::Pending)
    );
    assert_eq!(
        AnalysisTaskStatus::from_db("retry_waiting"),
        Some(AnalysisTaskStatus::RetryWaiting)
    );
    assert_eq!(AnalysisTaskStatus::from_db("unknown_status"), None);
    assert_eq!(AnalysisBarState::Closed.as_str(), "closed");
    assert_eq!(
        AnalysisBarState::from_db("closed"),
        Some(AnalysisBarState::Closed)
    );
    assert_eq!(AnalysisBarState::from_db("not_a_state"), None);
    assert_eq!(
        RetryPolicyClass::LlmStructuredOutput.as_str(),
        "llm_structured_output"
    );
    assert_eq!(PromptResultSemantics::SharedAsset.as_str(), "shared_asset");
    assert_eq!(spec.prompt_version, "v1");
    assert_eq!(spec.bar_state_support.len(), 2);
}

#[test]
fn app_error_retryable_is_conservative() {
    let deterministic_provider = pa_core::AppError::Provider {
        message: "failed to parse provider response".to_string(),
        source: Some(Box::new(std::io::Error::new(
            ErrorKind::InvalidData,
            "invalid json",
        ))),
    };
    assert!(!deterministic_provider.is_retryable());

    let transient_provider = pa_core::AppError::Provider {
        message: "provider request timed out".to_string(),
        source: Some(Box::new(std::io::Error::new(
            ErrorKind::TimedOut,
            "timeout",
        ))),
    };
    assert!(transient_provider.is_retryable());

    let deterministic_storage = pa_core::AppError::Storage {
        message: "failed to read config file at config.toml".to_string(),
        source: Some(Box::new(std::io::Error::new(
            ErrorKind::NotFound,
            "missing file",
        ))),
    };
    assert!(!deterministic_storage.is_retryable());
}
