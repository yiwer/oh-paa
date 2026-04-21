use std::collections::HashMap;

use pa_core::AppError;

use crate::{
    AnalysisStepSpec, ModelExecutionProfile, PromptSpec, PromptTemplateSpec, StepExecutionBinding,
};

#[derive(Debug)]
struct RegisteredStepSpec {
    spec: AnalysisStepSpec,
    output_validator: jsonschema::Validator,
}

#[derive(Debug)]
pub(crate) struct RegisteredPromptSpec<'a> {
    pub(crate) spec: &'a PromptTemplateSpec,
    pub(crate) output_validator: &'a jsonschema::Validator,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedStep<'a> {
    pub step: &'a AnalysisStepSpec,
    pub prompt: &'a PromptTemplateSpec,
    pub profile: &'a ModelExecutionProfile,
}

#[derive(Debug, Default)]
pub struct StepRegistry {
    steps: HashMap<(String, String), RegisteredStepSpec>,
    prompts: HashMap<(String, String), PromptTemplateSpec>,
    profiles: HashMap<String, ModelExecutionProfile>,
    bindings: HashMap<(String, String), StepExecutionBinding>,
}

impl StepRegistry {
    pub fn with_spec(self, spec: PromptSpec) -> Result<Self, AppError> {
        let step = AnalysisStepSpec {
            step_key: spec.prompt_key.clone(),
            step_version: spec.prompt_version.clone(),
            task_type: spec.task_type,
            input_schema_version: spec.input_schema_version,
            output_schema_version: spec.output_schema_version,
            output_json_schema: spec.output_json_schema,
            result_semantics: spec.result_semantics,
            bar_state_support: spec.bar_state_support,
            dependency_policy: "legacy_prompt_spec".to_string(),
        };
        let prompt = PromptTemplateSpec {
            step_key: spec.prompt_key,
            step_version: spec.prompt_version,
            system_prompt: spec.system_prompt,
            developer_instructions: Vec::new(),
        };

        self.with_step(step)?.with_prompt_template(prompt)
    }

    pub fn with_step(mut self, spec: AnalysisStepSpec) -> Result<Self, AppError> {
        let key = (spec.step_key.clone(), spec.step_version.clone());
        if self.steps.contains_key(&key) {
            return Err(AppError::Analysis {
                message: format!("duplicate step spec registration: {}:{}", key.0, key.1),
                source: None,
            });
        }

        let output_validator =
            jsonschema::validator_for(&spec.output_json_schema).map_err(|err| {
                AppError::Analysis {
                    message: format!(
                        "invalid output schema for {}:{}: {err}",
                        spec.step_key, spec.step_version
                    ),
                    source: None,
                }
            })?;

        self.steps.insert(
            key,
            RegisteredStepSpec {
                spec,
                output_validator,
            },
        );

        Ok(self)
    }

    pub fn with_prompt_template(mut self, prompt: PromptTemplateSpec) -> Result<Self, AppError> {
        let key = (prompt.step_key.clone(), prompt.step_version.clone());
        if self.prompts.contains_key(&key) {
            return Err(AppError::Analysis {
                message: format!(
                    "duplicate prompt template registration: {}:{}",
                    key.0, key.1
                ),
                source: None,
            });
        }

        self.prompts.insert(key, prompt);
        Ok(self)
    }

    pub fn with_execution_profile(
        mut self,
        profile: ModelExecutionProfile,
    ) -> Result<Self, AppError> {
        let key = profile.profile_key.clone();
        if self.profiles.contains_key(&key) {
            return Err(AppError::Analysis {
                message: format!("duplicate execution profile registration: {key}"),
                source: None,
            });
        }

        self.profiles.insert(key, profile);
        Ok(self)
    }

    pub fn with_binding(mut self, binding: StepExecutionBinding) -> Result<Self, AppError> {
        let key = (binding.step_key.clone(), binding.step_version.clone());
        if self.bindings.contains_key(&key) {
            return Err(AppError::Analysis {
                message: format!("duplicate step execution binding: {}:{}", key.0, key.1),
                source: None,
            });
        }

        self.bindings.insert(key, binding);
        Ok(self)
    }

    pub fn resolve(&self, step_key: &str, step_version: &str) -> Option<ResolvedStep<'_>> {
        let key = (step_key.to_owned(), step_version.to_owned());
        let binding = self.bindings.get(&key)?;
        let step = self.steps.get(&key)?;
        let prompt = self.prompts.get(&key)?;
        let profile = self.profiles.get(&binding.execution_profile)?;
        Some(ResolvedStep {
            step: &step.spec,
            prompt,
            profile,
        })
    }

    pub(crate) fn get(
        &self,
        prompt_key: &str,
        prompt_version: &str,
    ) -> Option<RegisteredPromptSpec<'_>> {
        let key = (prompt_key.to_owned(), prompt_version.to_owned());
        let step = self.steps.get(&key)?;
        let prompt = self.prompts.get(&key)?;
        Some(RegisteredPromptSpec {
            spec: prompt,
            output_validator: &step.output_validator,
        })
    }
}
