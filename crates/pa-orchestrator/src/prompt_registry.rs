use std::collections::HashMap;

use crate::PromptSpec;

#[derive(Debug, Clone, Default)]
pub struct PromptRegistry {
    specs: HashMap<(String, String), PromptSpec>,
}

impl PromptRegistry {
    pub fn with_spec(mut self, spec: PromptSpec) -> Self {
        self.specs.insert(
            (spec.prompt_key.clone(), spec.prompt_version.clone()),
            spec,
        );
        self
    }

    pub fn get(&self, prompt_key: &str, prompt_version: &str) -> Option<&PromptSpec> {
        self.specs
            .get(&(prompt_key.to_owned(), prompt_version.to_owned()))
    }
}
