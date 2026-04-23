#[allow(dead_code)]
#[path = "openai_client.rs"]
mod openai_client;

use async_trait::async_trait;
use pa_core::AppError;
use serde_json::Value;

pub use openai_client::{OpenAiCompatibleClient, OpenAiProviderRuntime};

pub const INVALID_JSON_RESPONSE_CONTENT_ERROR: &str =
    "chat completions response content was not valid JSON";

#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    pub provider: String,
    pub model: String,
    pub system_prompt: String,
    pub developer_instructions: Vec<String>,
    pub input_json: Value,
    pub output_json_schema: Option<Value>,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub structured_output_mode: StructuredOutputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredOutputMode {
    NativeJsonSchema,
    JsonObject,
    PromptEnforcedJson,
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
        let request_payload_json = request_payload_json(request);

        match &self.response {
            FixtureResponse::Success(output_json) => LlmCallEnvelope::Success(LlmSuccessEnvelope {
                llm_provider: request.provider.clone(),
                model: request.model.clone(),
                request_payload_json,
                raw_response_json: output_json.clone(),
                parsed_output_json: output_json.clone(),
            }),
            FixtureResponse::Failure {
                message,
                raw_response_json,
            } => LlmCallEnvelope::Failure(LlmFailureEnvelope {
                llm_provider: request.provider.clone(),
                model: request.model.clone(),
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

pub(crate) fn request_payload_json(request: &LlmRequest) -> Value {
    let mut payload = serde_json::Map::from_iter([
        (
            "provider".to_string(),
            Value::String(request.provider.clone()),
        ),
        ("model".to_string(), Value::String(request.model.clone())),
        (
            "system_prompt".to_string(),
            Value::String(request.system_prompt.clone()),
        ),
        (
            "developer_instructions".to_string(),
            Value::Array(
                request
                    .developer_instructions
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        ),
        ("input_json".to_string(), request.input_json.clone()),
        ("max_tokens".to_string(), Value::from(request.max_tokens)),
        (
            "timeout_secs".to_string(),
            Value::from(request.timeout_secs),
        ),
        (
            "structured_output_mode".to_string(),
            Value::String(structured_output_mode_name(request.structured_output_mode).to_string()),
        ),
    ]);

    if let Some(output_json_schema) = &request.output_json_schema {
        payload.insert("output_json_schema".to_string(), output_json_schema.clone());
    }

    Value::Object(payload)
}

pub(crate) fn structured_output_mode_name(mode: StructuredOutputMode) -> &'static str {
    match mode {
        StructuredOutputMode::NativeJsonSchema => "native_json_schema",
        StructuredOutputMode::JsonObject => "json_object",
        StructuredOutputMode::PromptEnforcedJson => "prompt_enforced_json",
    }
}
