use pa_core::AppError;
use serde_json::Value;

use crate::{LlmClient, PromptRegistry};

#[derive(Debug, Clone)]
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
    ) -> Result<Value, AppError> {
        let spec = self
            .prompt_registry
            .get(prompt_key, prompt_version)
            .ok_or_else(|| AppError::Analysis {
                message: format!("missing prompt spec: {prompt_key}:{prompt_version}"),
                source: None,
            })?;

        let output_json = self
            .llm_client
            .generate_json(&spec.system_prompt, input_json)
            .await?;

        let validator =
            jsonschema::validator_for(&spec.output_json_schema).map_err(|err| AppError::Analysis {
                message: format!("invalid output schema for {prompt_key}:{prompt_version}: {err}"),
                source: None,
            })?;

        if let Some(first) = validator.iter_errors(&output_json).next() {
            return Err(AppError::Analysis {
                message: format!("schema validation failed: {first}"),
                source: None,
            });
        }

        Ok(output_json)
    }
}
