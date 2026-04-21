use async_trait::async_trait;
use pa_core::AppError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct LlmResponse {
    pub llm_provider: String,
    pub model: String,
    pub raw_response_json: Value,
    pub parsed_output_json: Value,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_json(
        &self,
        system_prompt: &str,
        input_json: &Value,
    ) -> Result<LlmResponse, AppError>;
}

#[derive(Debug, Clone)]
pub struct FixtureLlmClient {
    response: LlmResponse,
}

impl FixtureLlmClient {
    pub fn with_json(output_json: Value) -> Self {
        Self {
            response: LlmResponse {
                llm_provider: "fixture".to_string(),
                model: "fixture-json".to_string(),
                raw_response_json: output_json.clone(),
                parsed_output_json: output_json,
            },
        }
    }

    pub fn with_response(response: LlmResponse) -> Self {
        Self { response }
    }
}

#[async_trait]
impl LlmClient for FixtureLlmClient {
    async fn generate_json(
        &self,
        _system_prompt: &str,
        _input_json: &Value,
    ) -> Result<LlmResponse, AppError> {
        Ok(self.response.clone())
    }
}
