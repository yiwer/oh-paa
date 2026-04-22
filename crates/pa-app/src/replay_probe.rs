use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use pa_core::AppError;
use pa_orchestrator::ExecutionOutcome;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    build_worker_executor_from_config,
    replay_config::load_replay_config,
    workspace_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeCliArgs {
    pub config_path: String,
    pub step_key: String,
    pub step_version: String,
    pub input_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub step_key: String,
    pub step_version: String,
    pub llm_provider: String,
    pub model: String,
    pub schema_valid: bool,
    pub failure_category: Option<String>,
    pub schema_validation_error: Option<String>,
    pub outbound_error_message: Option<String>,
    pub output_json: Value,
    pub raw_response_json: Option<Value>,
}

pub fn parse_probe_cli_args<I, S>(args: I) -> Result<ProbeCliArgs, anyhow::Error>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let values = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let config_path =
        value_after(&values, "--config").ok_or_else(|| missing_flag_error(&values, "--config"))?;
    let step = value_after(&values, "--step").ok_or_else(|| missing_flag_error(&values, "--step"))?;
    let input_path =
        value_after(&values, "--input").ok_or_else(|| missing_flag_error(&values, "--input"))?;
    let (step_key, step_version) = parse_step_selector(&step)?;

    Ok(ProbeCliArgs {
        config_path,
        step_key,
        step_version,
        input_path,
    })
}

pub async fn run_probe_from_path(
    config_path: impl AsRef<Path>,
    step_key: &str,
    step_version: &str,
    input_path: impl AsRef<Path>,
) -> Result<ProbeResult, AppError> {
    let resolved_config_path = resolve_input_path(config_path);
    let resolved_input_path = resolve_input_path(input_path);
    let resolved = load_replay_config(resolved_config_path)?;
    let executor = build_worker_executor_from_config(&resolved.app_config)?;
    let input_json = read_json_from_path(&resolved_input_path)?;
    let outcome = executor
        .execute_json(step_key, step_version, &input_json)
        .await?;

    Ok(probe_result_from_outcome(step_key, step_version, outcome))
}

fn read_json_from_path(path: &Path) -> Result<Value, AppError> {
    let raw = fs::read_to_string(path).map_err(|err| AppError::Storage {
        message: format!("failed to read replay probe input from {}", path.display()),
        source: Some(Box::new(err)),
    })?;

    serde_json::from_str(&raw).map_err(|err| AppError::Validation {
        message: format!(
            "failed to deserialize replay probe input from {}",
            path.display()
        ),
        source: Some(Box::new(err)),
    })
}

fn probe_result_from_outcome(
    step_key: &str,
    step_version: &str,
    outcome: ExecutionOutcome,
) -> ProbeResult {
    match outcome {
        ExecutionOutcome::Success(attempt) => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: true,
            failure_category: None,
            schema_validation_error: None,
            outbound_error_message: None,
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
        ExecutionOutcome::SchemaValidationFailed(attempt) => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: false,
            failure_category: Some("schema_validation".to_string()),
            schema_validation_error: attempt.schema_validation_error,
            outbound_error_message: None,
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
        ExecutionOutcome::OutboundCallFailed { attempt, .. } => ProbeResult {
            step_key: step_key.to_string(),
            step_version: step_version.to_string(),
            llm_provider: attempt.llm_provider,
            model: attempt.model,
            schema_valid: false,
            failure_category: Some("transport".to_string()),
            schema_validation_error: None,
            outbound_error_message: attempt.outbound_error_message,
            output_json: attempt.parsed_output_json.unwrap_or(Value::Null),
            raw_response_json: attempt.raw_response_json,
        },
    }
}

fn resolve_input_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }

    workspace_root().join(path)
}

fn parse_step_selector(value: &str) -> Result<(String, String), anyhow::Error> {
    let (step_key, step_version) = value
        .split_once(':')
        .ok_or_else(|| anyhow!("missing step version in --step {value}"))?;
    if step_key.trim().is_empty() {
        return Err(anyhow!("missing step key in --step {value}"));
    }
    if step_version.trim().is_empty() {
        return Err(anyhow!("missing step version in --step {value}"));
    }

    Ok((step_key.to_string(), step_version.to_string()))
}

fn missing_flag_error(values: &[String], flag: &str) -> anyhow::Error {
    if has_flag(values, flag) {
        anyhow!("missing {flag}")
    } else {
        anyhow!("missing required {flag}")
    }
}

fn value_after(values: &[String], flag: &str) -> Option<String> {
    values
        .iter()
        .position(|value| value == flag)
        .and_then(|index| values.get(index + 1))
        .filter(|value| !value.starts_with("--"))
        .cloned()
}

fn has_flag(values: &[String], flag: &str) -> bool {
    values.iter().any(|value| value == flag)
}

#[cfg(test)]
mod tests {
    use pa_orchestrator::ExecutionAttempt;
    use serde_json::json;

    use super::probe_result_from_outcome;

    #[test]
    fn probe_result_maps_schema_validation_outcome() {
        let outcome = pa_orchestrator::ExecutionOutcome::SchemaValidationFailed(ExecutionAttempt {
            llm_provider: "dashscope".to_string(),
            model: "qwen-plus".to_string(),
            request_payload_json: json!({}),
            raw_response_json: Some(json!({"raw": true})),
            parsed_output_json: Some(json!({"partial": true})),
            schema_validation_error: Some("missing required property".to_string()),
            outbound_error_message: None,
        });

        let result = probe_result_from_outcome("shared_pa_state_bar", "v1", outcome);

        assert_eq!(result.failure_category.as_deref(), Some("schema_validation"));
        assert!(!result.schema_valid);
        assert_eq!(
            result.schema_validation_error.as_deref(),
            Some("missing required property")
        );
        assert_eq!(result.outbound_error_message, None);
        assert_eq!(result.output_json["partial"], true);
        assert_eq!(result.raw_response_json, Some(json!({"raw": true})));
    }

    #[test]
    fn probe_result_maps_outbound_failure_to_transport_category() {
        let attempt = ExecutionAttempt {
            llm_provider: "dashscope".to_string(),
            model: "qwen-plus".to_string(),
            request_payload_json: json!({}),
            raw_response_json: Some(json!({"raw": false})),
            parsed_output_json: None,
            schema_validation_error: None,
            outbound_error_message: Some("timeout".to_string()),
        };
        let outcome = pa_orchestrator::ExecutionOutcome::OutboundCallFailed {
            attempt,
            error: AppError::Analysis {
                message: "transport failed".to_string(),
                source: None,
            },
        };

        let result = probe_result_from_outcome("shared_pa_state_bar", "v1", outcome);

        assert_eq!(result.failure_category.as_deref(), Some("transport"));
        assert!(!result.schema_valid);
        assert_eq!(result.schema_validation_error, None);
        assert_eq!(result.outbound_error_message.as_deref(), Some("timeout"));
        assert_eq!(result.output_json, Value::Null);
    }
}
