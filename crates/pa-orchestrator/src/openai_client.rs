use std::{collections::BTreeMap, time::Duration};

use async_trait::async_trait;
use pa_core::AppError;
use serde_json::{Value, json};

use super::{
    LlmCallEnvelope, LlmClient, LlmFailureEnvelope, LlmRequest, LlmSuccessEnvelope,
    StructuredOutputMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiProviderRuntime {
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    providers: BTreeMap<String, OpenAiProviderRuntime>,
}

impl OpenAiCompatibleClient {
    pub fn new(providers: BTreeMap<String, OpenAiProviderRuntime>) -> Self {
        Self {
            http: reqwest::Client::new(),
            providers,
        }
    }

    fn build_payload(&self, request: &LlmRequest) -> Value {
        json!({
            "model": request.model,
            "messages": build_messages(request),
            "max_tokens": request.max_tokens,
            "response_format": response_format_for(request.structured_output_mode),
        })
    }

    async fn post_chat_completions(&self, request: &LlmRequest) -> Result<Value, AppError> {
        let provider =
            self.providers
                .get(&request.provider)
                .ok_or_else(|| AppError::Validation {
                    message: format!("missing llm provider runtime: {}", request.provider),
                    source: None,
                })?;

        let payload = self.build_payload(request);

        let response = self
            .http
            .post(format!(
                "{}/chat/completions",
                provider.base_url.trim_end_matches('/')
            ))
            .bearer_auth(&provider.api_key)
            .timeout(Duration::from_secs(request.timeout_secs))
            .json(&payload)
            .send()
            .await
            .map_err(provider_error)?
            .error_for_status()
            .map_err(provider_error)?;

        response.json::<Value>().await.map_err(provider_error)
    }

    fn parse_response_json(&self, raw_response_json: &Value) -> Result<Value, AppError> {
        let content = raw_response_json
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .ok_or_else(|| AppError::Provider {
                message: "chat completions response missing choices[0].message.content".to_string(),
                source: None,
            })?;

        match content {
            Value::String(text) => serde_json::from_str(text).map_err(|err| AppError::Provider {
                message: "chat completions response content was not valid JSON".to_string(),
                source: Some(Box::new(err)),
            }),
            Value::Array(parts) => {
                let joined = parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<String>();
                serde_json::from_str(&joined).map_err(|err| AppError::Provider {
                    message: "chat completions response content parts were not valid JSON"
                        .to_string(),
                    source: Some(Box::new(err)),
                })
            }
            other if other.is_object() || other.is_array() => Ok(other.clone()),
            _ => Err(AppError::Provider {
                message: "chat completions response content had unsupported shape".to_string(),
                source: None,
            }),
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    async fn generate_json(&self, request: &LlmRequest) -> LlmCallEnvelope {
        let request_payload_json = self.build_payload(request);

        match self.post_chat_completions(request).await {
            Ok(raw_response_json) => match self.parse_response_json(&raw_response_json) {
                Ok(parsed_output_json) => LlmCallEnvelope::Success(LlmSuccessEnvelope {
                    llm_provider: request.provider.clone(),
                    model: request.model.clone(),
                    request_payload_json,
                    raw_response_json,
                    parsed_output_json,
                }),
                Err(error) => LlmCallEnvelope::Failure(LlmFailureEnvelope {
                    llm_provider: request.provider.clone(),
                    model: request.model.clone(),
                    request_payload_json,
                    raw_response_json: Some(raw_response_json),
                    error,
                }),
            },
            Err(error) => LlmCallEnvelope::Failure(LlmFailureEnvelope {
                llm_provider: request.provider.clone(),
                model: request.model.clone(),
                request_payload_json,
                raw_response_json: None,
                error,
            }),
        }
    }
}

fn build_messages(request: &LlmRequest) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "system",
        "content": request.system_prompt,
    })];

    for instruction in &request.developer_instructions {
        messages.push(json!({
            "role": "developer",
            "content": instruction,
        }));
    }

    if matches!(
        request.structured_output_mode,
        StructuredOutputMode::PromptEnforcedJson
    ) {
        messages.push(json!({
            "role": "developer",
            "content": "Return only valid JSON with no markdown or prose.",
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": request.input_json.to_string(),
    }));

    messages
}

fn response_format_for(mode: StructuredOutputMode) -> Value {
    match mode {
        StructuredOutputMode::NativeJsonSchema => json!({
            "type": "json_schema",
            "json_schema": {
                "name": "structured_output",
                "schema": {
                    "type": "object"
                }
            }
        }),
        StructuredOutputMode::JsonObject => json!({
            "type": "json_object"
        }),
        StructuredOutputMode::PromptEnforcedJson => Value::Null,
    }
}

fn provider_error(error: reqwest::Error) -> AppError {
    AppError::Provider {
        message: error.to_string(),
        source: Some(Box::new(error)),
    }
}
