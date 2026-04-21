use pa_core::AppError;
use serde_json::Value;

use crate::{LlmClient, PromptRegistry};

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionAttempt {
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Value,
    pub parsed_output_json: Value,
    pub schema_validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    Success(ExecutionAttempt),
    SchemaValidationFailed(ExecutionAttempt),
}

#[derive(Debug)]
pub struct Executor<C> {
    prompt_registry: PromptRegistry,
    llm_client: C,
}

impl<C> Executor<C>
where
    C: LlmClient,
{
    pub fn new(prompt_registry: PromptRegistry, llm_client: C) -> Self {
        Self {
            prompt_registry,
            llm_client,
        }
    }

    pub async fn execute_json(
        &self,
        prompt_key: &str,
        prompt_version: &str,
        input_json: &Value,
    ) -> Result<ExecutionOutcome, AppError> {
        let registered_spec = self
            .prompt_registry
            .get(prompt_key, prompt_version)
            .ok_or_else(|| AppError::Analysis {
                message: format!("missing prompt spec: {prompt_key}:{prompt_version}"),
                source: None,
            })?;

        let llm_response = self
            .llm_client
            .generate_json(&registered_spec.spec.system_prompt, input_json)
            .await?;

        let mut attempt = ExecutionAttempt {
            llm_provider: llm_response.llm_provider,
            model: llm_response.model,
            request_payload_json: serde_json::json!({
                "system_prompt": &registered_spec.spec.system_prompt,
                "input_json": input_json,
            }),
            raw_response_json: llm_response.raw_response_json,
            parsed_output_json: llm_response.parsed_output_json,
            schema_validation_error: None,
        };

        let first_schema_error = registered_spec
            .output_validator
            .iter_errors(&attempt.parsed_output_json)
            .next()
            .map(|error| error.to_string());

        if let Some(first) = first_schema_error {
            attempt.schema_validation_error = Some(first);
            return Ok(ExecutionOutcome::SchemaValidationFailed(attempt));
        }

        Ok(ExecutionOutcome::Success(attempt))
    }
}
