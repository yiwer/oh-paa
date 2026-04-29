use pa_core::AppError;
use serde_json::Value;

use crate::llm::StructuredOutputMode;
use crate::prompt_registry::{RegisteredPromptSpec, RegistrationProvenance};
use crate::{LlmCallEnvelope, LlmClient, LlmRequest, ModelExecutionProfile, StepRegistry};

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
    step_registry: StepRegistry,
    llm_client: C,
}

impl<C> Executor<C>
where
    C: LlmClient,
{
    pub fn new(step_registry: StepRegistry, llm_client: C) -> Self {
        Self {
            step_registry,
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
            .step_registry
            .get(prompt_key, prompt_version)
            .ok_or_else(|| AppError::Analysis {
                message: format!("missing step registration: {prompt_key}:{prompt_version}"),
                source: None,
            })?;

        let resolved = match self.step_registry.resolve(prompt_key, prompt_version) {
            Some(resolved) => resolved,
            None if is_legacy_prompt_spec_registration(&registered_spec) => {
                return self
                    .execute_legacy_prompt_spec(registered_spec, input_json)
                    .await;
            }
            None => {
                return Err(AppError::Analysis {
                    message: format!(
                        "missing execution profile binding for step registration: {prompt_key}:{prompt_version}"
                    ),
                    source: None,
                });
            }
        };

        let llm_request = LlmRequest {
            provider: resolved.profile.provider.clone(),
            model: resolved.profile.model.clone(),
            system_prompt: resolved.prompt.system_prompt.clone(),
            developer_instructions: resolved.prompt.developer_instructions.clone(),
            input_json: input_json.clone(),
            output_json_schema: Some(resolved.step.output_json_schema.clone()),
            max_tokens: resolved.profile.max_tokens,
            timeout_secs: resolved.profile.timeout_secs,
            structured_output_mode: choose_structured_output_mode(resolved.profile),
            supports_reasoning: resolved.profile.supports_reasoning,
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

    async fn execute_legacy_prompt_spec(
        &self,
        registered_spec: RegisteredPromptSpec<'_>,
        input_json: &Value,
    ) -> Result<ExecutionOutcome, AppError> {
        let llm_request = LlmRequest {
            provider: "legacy-prompt-spec".to_string(),
            model: "legacy-prompt-spec".to_string(),
            system_prompt: registered_spec.spec.system_prompt.clone(),
            developer_instructions: registered_spec.spec.developer_instructions.clone(),
            input_json: input_json.clone(),
            output_json_schema: None,
            max_tokens: 0,
            timeout_secs: 0,
            structured_output_mode: StructuredOutputMode::PromptEnforcedJson,
            supports_reasoning: false,
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

fn choose_structured_output_mode(profile: &ModelExecutionProfile) -> StructuredOutputMode {
    if profile.supports_json_schema {
        StructuredOutputMode::NativeJsonSchema
    } else {
        StructuredOutputMode::JsonObject
    }
}

fn is_legacy_prompt_spec_registration(registered_spec: &RegisteredPromptSpec<'_>) -> bool {
    matches!(
        registered_spec.provenance,
        RegistrationProvenance::LegacyPromptSpec
    )
}
