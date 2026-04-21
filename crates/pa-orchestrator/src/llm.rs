use async_trait::async_trait;
use pa_core::AppError;
use serde_json::Value;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_json(&self, system_prompt: &str, input_json: &Value) -> Result<Value, AppError>;
}

#[derive(Debug, Clone)]
pub struct FixtureLlmClient {
    output_json: Value,
}

impl FixtureLlmClient {
    pub fn with_json(output_json: Value) -> Self {
        Self { output_json }
    }
}

#[async_trait]
impl LlmClient for FixtureLlmClient {
    async fn generate_json(
        &self,
        _system_prompt: &str,
        _input_json: &Value,
    ) -> Result<Value, AppError> {
        Ok(self.output_json.clone())
    }
}
