use async_trait::async_trait;
use pa_core::AppError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub input_json: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmSuccessEnvelope {
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Value,
    pub parsed_output_json: Value,
}

#[derive(Debug)]
pub struct LlmFailureEnvelope {
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Option<Value>,
    pub error: AppError,
}

#[derive(Debug)]
pub enum LlmCallEnvelope {
    Success(LlmSuccessEnvelope),
    Failure(LlmFailureEnvelope),
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_json(&self, request: &LlmRequest) -> LlmCallEnvelope;
}

#[derive(Debug)]
pub struct FixtureLlmClient {
    response: FixtureResponse,
}

#[derive(Debug, Clone)]
enum FixtureResponse {
    Success(Value),
    Failure {
        message: String,
        raw_response_json: Option<Value>,
    },
}

impl FixtureLlmClient {
    pub fn with_json(output_json: Value) -> Self {
        Self {
            response: FixtureResponse::Success(output_json),
        }
    }

    pub fn with_provider_error(message: impl Into<String>) -> Self {
        Self {
            response: FixtureResponse::Failure {
                message: message.into(),
                raw_response_json: None,
            },
        }
    }
}

#[async_trait]
impl LlmClient for FixtureLlmClient {
    async fn generate_json(&self, request: &LlmRequest) -> LlmCallEnvelope {
        let request_payload_json = serde_json::json!({
            "system_prompt": request.system_prompt,
            "input_json": request.input_json,
        });

        match &self.response {
            FixtureResponse::Success(output_json) => LlmCallEnvelope::Success(LlmSuccessEnvelope {
                llm_provider: "fixture".to_string(),
                model: "fixture-json".to_string(),
                request_payload_json,
                raw_response_json: output_json.clone(),
                parsed_output_json: output_json.clone(),
            }),
            FixtureResponse::Failure {
                message,
                raw_response_json,
            } => LlmCallEnvelope::Failure(LlmFailureEnvelope {
                llm_provider: "fixture".to_string(),
                model: "fixture-json".to_string(),
                request_payload_json,
                raw_response_json: raw_response_json.clone(),
                error: AppError::Provider {
                    message: message.clone(),
                    source: None,
                },
            }),
        }
    }
}
