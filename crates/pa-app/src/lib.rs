#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use pa_core::{AppConfig, AppError, config::OpenAiApiStyle};
use pa_orchestrator::{
    Executor, ModelExecutionProfile, OpenAiCompatibleClient, OpenAiProviderRuntime,
    StepExecutionBinding, StepRegistry,
};

pub fn build_openai_provider_runtimes(
    config: &AppConfig,
) -> BTreeMap<String, OpenAiProviderRuntime> {
    config
        .llm
        .providers
        .iter()
        .map(|(provider_key, provider)| {
            let runtime = match provider.openai_api_style {
                OpenAiApiStyle::ChatCompletions => OpenAiProviderRuntime {
                    base_url: provider.base_url.clone(),
                    api_key: provider.api_key.clone(),
                },
            };

            (provider_key.clone(), runtime)
        })
        .collect()
}

pub fn build_step_registry_from_config(config: &AppConfig) -> Result<StepRegistry, AppError> {
    let mut registry = StepRegistry::default();

    for (profile_key, profile) in &config.llm.execution_profiles {
        registry = registry.with_execution_profile(ModelExecutionProfile {
            profile_key: profile_key.clone(),
            provider: profile.provider.clone(),
            model: profile.model.clone(),
            max_tokens: profile.max_tokens,
            timeout_secs: profile.per_call_timeout_secs,
            max_retries: profile.max_retries,
            retry_initial_backoff_ms: profile.retry_initial_backoff_ms,
            supports_json_schema: profile.supports_json_schema,
            supports_reasoning: profile.supports_reasoning,
        })?;
    }

    for (step, prompt) in pipeline_step_catalog() {
        let binding_name = step_binding_name(&step.step_key, &step.step_version);
        let binding_config = config.llm.step_bindings.get(&binding_name).ok_or_else(|| {
            missing_step_binding_error(&binding_name, &step.step_key, &step.step_version)
        })?;
        let binding = StepExecutionBinding {
            step_key: step.step_key.clone(),
            step_version: step.step_version.clone(),
            execution_profile: binding_config.execution_profile.clone(),
        };

        registry = registry
            .with_step(step)?
            .with_prompt_template(prompt)?
            .with_binding(binding)?;
    }

    Ok(registry)
}

pub fn build_worker_executor_from_config(
    config: &AppConfig,
) -> Result<Executor<OpenAiCompatibleClient>, AppError> {
    let step_registry = build_step_registry_from_config(config)?;
    let provider_runtimes = build_openai_provider_runtimes(config);

    Ok(Executor::new(
        step_registry,
        OpenAiCompatibleClient::new(provider_runtimes),
    ))
}

fn pipeline_step_catalog() -> Vec<(
    pa_orchestrator::AnalysisStepSpec,
    pa_orchestrator::PromptTemplateSpec,
)> {
    vec![
        (
            pa_analysis::shared_pa_state_bar_v1(),
            pa_analysis::shared_pa_state_bar_prompt_v1(),
        ),
        (
            pa_analysis::shared_bar_analysis_v2(),
            pa_analysis::shared_bar_analysis_prompt_v2(),
        ),
        (
            pa_analysis::shared_daily_context_v2(),
            pa_analysis::shared_daily_context_prompt_v2(),
        ),
        (
            pa_user::user_position_advice_v2(),
            pa_user::user_position_advice_prompt_v2(),
        ),
    ]
}

fn step_binding_name(step_key: &str, step_version: &str) -> String {
    format!("{step_key}_{step_version}")
}

fn missing_step_binding_error(binding_name: &str, step_key: &str, step_version: &str) -> AppError {
    AppError::Validation {
        message: format!(
            "missing llm.step_bindings.{binding_name} for required step {step_key}:{step_version}"
        ),
        source: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pa_core::{
        AppConfig,
        config::{
            LlmConfig, LlmExecutionProfileConfig, LlmProviderConfig, LlmStepBindingConfig,
            OpenAiApiStyle,
        },
    };

    use super::{build_openai_provider_runtimes, build_step_registry_from_config};

    #[test]
    fn build_openai_provider_runtimes_maps_each_configured_provider() {
        let config = sample_config();

        let runtimes = build_openai_provider_runtimes(&config);

        assert_eq!(runtimes.len(), 2);
        assert_eq!(
            runtimes["dashscope"].base_url,
            "https://dashscope.example/v1"
        );
        assert_eq!(runtimes["dashscope"].api_key, "dashscope-key");
        assert_eq!(runtimes["deepseek"].base_url, "https://deepseek.example/v1");
        assert_eq!(runtimes["deepseek"].api_key, "deepseek-key");
    }

    #[test]
    fn build_step_registry_from_config_registers_expected_pipeline_steps() {
        let config = sample_config();

        let registry = build_step_registry_from_config(&config).expect("registry should build");

        let pa_state = registry
            .resolve("shared_pa_state_bar", "v1")
            .expect("shared PA-state step should resolve");
        assert_eq!(pa_state.profile.profile_key, "pa_state_extract_fast");
        assert_eq!(pa_state.binding.execution_profile, "pa_state_extract_fast");

        let shared_bar = registry
            .resolve("shared_bar_analysis", "v2")
            .expect("shared bar step should resolve");
        assert_eq!(shared_bar.profile.profile_key, "shared_bar_reasoner");
        assert_eq!(shared_bar.binding.execution_profile, "shared_bar_reasoner");

        let shared_daily = registry
            .resolve("shared_daily_context", "v2")
            .expect("shared daily step should resolve");
        assert_eq!(shared_daily.profile.profile_key, "shared_bar_reasoner");
        assert_eq!(
            shared_daily.binding.execution_profile,
            "shared_bar_reasoner"
        );

        let user_advice = registry
            .resolve("user_position_advice", "v2")
            .expect("user advice step should resolve");
        assert_eq!(user_advice.profile.profile_key, "user_position_reasoner");
        assert_eq!(
            user_advice.binding.execution_profile,
            "user_position_reasoner"
        );
    }

    #[test]
    fn build_step_registry_from_config_rejects_missing_required_binding() {
        let mut config = sample_config();
        config.llm.step_bindings.remove("user_position_advice_v2");

        let error =
            build_step_registry_from_config(&config).expect_err("missing binding should fail");

        match error {
            pa_core::AppError::Validation { message, source } => {
                assert!(message.contains("llm.step_bindings.user_position_advice_v2"));
                assert!(message.contains("user_position_advice:v2"));
                assert!(source.is_none());
            }
            other => panic!("expected validation error, got {other}"),
        }
    }

    #[test]
    fn build_step_registry_from_example_config_registers_all_runtime_steps() {
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config.example.toml");
        let config = AppConfig::load_from_path(config_path).expect("example config should parse");

        let registry = build_step_registry_from_config(&config).expect("registry should build");

        assert!(registry.resolve("shared_pa_state_bar", "v1").is_some());
        assert!(registry.resolve("shared_bar_analysis", "v2").is_some());
        assert!(registry.resolve("shared_daily_context", "v2").is_some());
        assert!(registry.resolve("user_position_advice", "v2").is_some());
    }

    fn sample_config() -> AppConfig {
        AppConfig {
            database_url: "postgres://postgres:pgsql@localhost:5432/oh_paa".to_string(),
            server_addr: "127.0.0.1:3000".to_string(),
            eastmoney_base_url: "https://push2his.eastmoney.com/".to_string(),
            twelvedata_base_url: "https://api.twelvedata.com/".to_string(),
            twelvedata_api_key: "replace-with-real-key".to_string(),
            llm: LlmConfig {
                providers: BTreeMap::from([
                    (
                        "dashscope".to_string(),
                        LlmProviderConfig {
                            base_url: "https://dashscope.example/v1".to_string(),
                            api_key: "dashscope-key".to_string(),
                            openai_api_style: OpenAiApiStyle::ChatCompletions,
                        },
                    ),
                    (
                        "deepseek".to_string(),
                        LlmProviderConfig {
                            base_url: "https://deepseek.example/v1".to_string(),
                            api_key: "deepseek-key".to_string(),
                            openai_api_style: OpenAiApiStyle::ChatCompletions,
                        },
                    ),
                ]),
                execution_profiles: BTreeMap::from([
                    (
                        "pa_state_extract_fast".to_string(),
                        LlmExecutionProfileConfig {
                            provider: "dashscope".to_string(),
                            model: "qwen-plus".to_string(),
                            max_tokens: 12000,
                            max_retries: 2,
                            per_call_timeout_secs: 180,
                            retry_initial_backoff_ms: 1000,
                            supports_json_schema: false,
                            supports_reasoning: false,
                        },
                    ),
                    (
                        "shared_bar_reasoner".to_string(),
                        LlmExecutionProfileConfig {
                            provider: "deepseek".to_string(),
                            model: "deepseek-reasoner".to_string(),
                            max_tokens: 32768,
                            max_retries: 2,
                            per_call_timeout_secs: 600,
                            retry_initial_backoff_ms: 1000,
                            supports_json_schema: false,
                            supports_reasoning: true,
                        },
                    ),
                    (
                        "user_position_reasoner".to_string(),
                        LlmExecutionProfileConfig {
                            provider: "deepseek".to_string(),
                            model: "deepseek-chat".to_string(),
                            max_tokens: 16384,
                            max_retries: 2,
                            per_call_timeout_secs: 300,
                            retry_initial_backoff_ms: 1000,
                            supports_json_schema: false,
                            supports_reasoning: false,
                        },
                    ),
                ]),
                step_bindings: BTreeMap::from([
                    (
                        "shared_pa_state_bar_v1".to_string(),
                        LlmStepBindingConfig {
                            execution_profile: "pa_state_extract_fast".to_string(),
                        },
                    ),
                    (
                        "shared_bar_analysis_v2".to_string(),
                        LlmStepBindingConfig {
                            execution_profile: "shared_bar_reasoner".to_string(),
                        },
                    ),
                    (
                        "shared_daily_context_v2".to_string(),
                        LlmStepBindingConfig {
                            execution_profile: "shared_bar_reasoner".to_string(),
                        },
                    ),
                    (
                        "user_position_advice_v2".to_string(),
                        LlmStepBindingConfig {
                            execution_profile: "user_position_reasoner".to_string(),
                        },
                    ),
                ]),
            },
        }
    }
}
