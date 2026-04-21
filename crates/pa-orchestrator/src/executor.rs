use pa_core::AppError;
use serde_json::Value;

use crate::{LlmCallEnvelope, LlmClient, LlmRequest, PromptRegistry};

#[derive(Debug, Clone)]
pub struct ExecutionAttempt {
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Option<Value>,
    pub parsed_output_json: Option<Value>,
    pub schema_validation_error: Option<String>,
    pub outbound_error_message: Option<String>,
}

#[derive(Debug)]
pub enum ExecutionOutcome {
    Success(ExecutionAttempt),
    SchemaValidationFailed(ExecutionAttempt),
    OutboundCallFailed {
        attempt: ExecutionAttempt,
        error: AppError,
    },
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

        let llm_request = LlmRequest {
            system_prompt: registered_spec.spec.system_prompt.clone(),
            input_json: input_json.clone(),
        };

        let llm_outcome = self.llm_client.generate_json(&llm_request).await;

        match llm_outcome {
            LlmCallEnvelope::Failure(failed_call) => {
                let attempt = ExecutionAttempt {
                    llm_provider: failed_call.llm_provider,
                    model: failed_call.model,
                    request_payload_json: failed_call.request_payload_json,
                    raw_response_json: failed_call.raw_response_json,
                    parsed_output_json: None,
                    schema_validation_error: None,
                    outbound_error_message: Some(failed_call.error.to_string()),
                };
                Ok(ExecutionOutcome::OutboundCallFailed {
                    attempt,
                    error: failed_call.error,
                })
            }
            LlmCallEnvelope::Success(successful_call) => {
                let mut attempt = ExecutionAttempt {
                    llm_provider: successful_call.llm_provider,
                    model: successful_call.model,
                    request_payload_json: successful_call.request_payload_json,
                    raw_response_json: Some(successful_call.raw_response_json),
                    parsed_output_json: Some(successful_call.parsed_output_json),
                    schema_validation_error: None,
                    outbound_error_message: None,
                };

                let parsed_output_json = attempt
                    .parsed_output_json
                    .as_ref()
                    .expect("success payload should include parsed_output_json");

                let first_schema_error = registered_spec
                    .output_validator
                    .iter_errors(parsed_output_json)
                    .next()
                    .map(|error| error.to_string());

                if let Some(first) = first_schema_error {
                    attempt.schema_validation_error = Some(first);
                    return Ok(ExecutionOutcome::SchemaValidationFailed(attempt));
                }

                Ok(ExecutionOutcome::Success(attempt))
            }
        }
    }
}
