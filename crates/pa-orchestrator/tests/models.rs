use pa_orchestrator::{
    AnalysisBarState, AnalysisStepSpec, AnalysisTaskStatus, ModelExecutionProfile,
    PromptResultSemantics, PromptTemplateSpec, RetryPolicyClass, StepExecutionBinding,
    StepRegistry,
};
use std::io::ErrorKind;

#[test]
fn step_contract_models_expose_phase2_fields() {
    let step = AnalysisStepSpec {
        step_key: "shared_bar_analysis".into(),
        step_version: "v1".into(),
        task_type: "shared_bar_analysis".into(),
        input_schema_version: "v1".into(),
        output_schema_version: "v1".into(),
        output_json_schema: serde_json::json!({"type": "object"}),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
        dependency_policy: "market_runtime_only".into(),
    };
    let template = PromptTemplateSpec {
        step_key: "shared_bar_analysis".into(),
        step_version: "v1".into(),
        system_prompt: "Return JSON only".into(),
        developer_instructions: vec!["Do not invent data".into()],
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
    assert_eq!(step.step_version, "v1");
    assert_eq!(step.bar_state_support.len(), 2);
    assert_eq!(template.developer_instructions.len(), 1);
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

#[test]
fn step_registry_resolves_step_template_profile_and_binding() {
    let registry = StepRegistry::default()
        .with_step(AnalysisStepSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            task_type: "shared_pa_state_bar".into(),
            input_schema_version: "v1".into(),
            output_schema_version: "v1".into(),
            output_json_schema: serde_json::json!({"type":"object"}),
            result_semantics: PromptResultSemantics::SharedAsset,
            bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
            dependency_policy: "market_runtime_only".into(),
        })
        .unwrap()
        .with_prompt_template(PromptTemplateSpec {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            system_prompt: "Return JSON".into(),
            developer_instructions: vec!["Do not invent data".into()],
        })
        .unwrap()
        .with_execution_profile(ModelExecutionProfile {
            profile_key: "pa_state_extract_fast".into(),
            provider: "dashscope".into(),
            model: "qwen-plus".into(),
            max_tokens: 12000,
            timeout_secs: 180,
            max_retries: 2,
            retry_initial_backoff_ms: 1000,
            supports_json_schema: false,
            supports_reasoning: false,
        })
        .unwrap()
        .with_binding(StepExecutionBinding {
            step_key: "shared_pa_state_bar".into(),
            step_version: "v1".into(),
            execution_profile: "pa_state_extract_fast".into(),
        })
        .unwrap();

    let resolved = registry.resolve("shared_pa_state_bar", "v1").unwrap();
    assert_eq!(resolved.step.task_type, "shared_pa_state_bar");
    assert_eq!(resolved.profile.model, "qwen-plus");
    assert_eq!(resolved.prompt.developer_instructions.len(), 1);
}
