use std::collections::HashMap;

use pa_core::AppError;

use crate::PromptSpec;

#[derive(Debug)]
pub(crate) struct RegisteredPromptSpec {
    pub(crate) spec: PromptSpec,
    pub(crate) output_validator: jsonschema::Validator,
}

#[derive(Debug, Default)]
pub struct PromptRegistry {
    specs: HashMap<(String, String), RegisteredPromptSpec>,
}

impl PromptRegistry {
    pub fn with_spec(mut self, spec: PromptSpec) -> Result<Self, AppError> {
        let key = (spec.prompt_key.clone(), spec.prompt_version.clone());
        if self.specs.contains_key(&key) {
            return Err(AppError::Analysis {
                message: format!("duplicate prompt spec registration: {}:{}", key.0, key.1),
                source: None,
            });
        }

        let output_validator =
            jsonschema::validator_for(&spec.output_json_schema).map_err(|err| AppError::Analysis {
                message: format!(
                    "invalid output schema for {}:{}: {err}",
                    spec.prompt_key, spec.prompt_version
                ),
                source: None,
            })?;

        self.specs.insert(
            key,
            RegisteredPromptSpec {
                spec,
                output_validator,
            },
        );

        Ok(self)
    }

    pub(crate) fn get(&self, prompt_key: &str, prompt_version: &str) -> Option<&RegisteredPromptSpec> {
        self.specs
            .get(&(prompt_key.to_owned(), prompt_version.to_owned()))
    }
}
