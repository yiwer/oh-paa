use std::{fs, path::Path};

use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub eastmoney_base_url: String,
    pub twelvedata_base_url: String,
    pub twelvedata_api_key: String,
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmConfig {
    pub providers: std::collections::BTreeMap<String, LlmProviderConfig>,
    pub execution_profiles: std::collections::BTreeMap<String, LlmExecutionProfileConfig>,
    pub step_bindings: std::collections::BTreeMap<String, LlmStepBindingConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub openai_api_style: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmExecutionProfileConfig {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub max_retries: u32,
    pub per_call_timeout_secs: u64,
    pub retry_initial_backoff_ms: u64,
    pub supports_json_schema: bool,
    pub supports_reasoning: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LlmStepBindingConfig {
    pub execution_profile: String,
}

impl AppConfig {
    pub fn load() -> Result<AppConfig, AppError> {
        let path = std::env::current_dir()
            .map_err(|source| AppError::Storage {
                message: "failed to resolve current working directory".to_string(),
                source: Some(Box::new(source)),
            })?
            .join("config.toml");

        Self::load_from_path(path)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<AppConfig, AppError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| AppError::Storage {
            message: format!("failed to read config file at {}", path.display()),
            source: Some(Box::new(source)),
        })?;

        toml::from_str(&raw).map_err(|source| AppError::Validation {
            message: format!("failed to parse config file at {}", path.display()),
            source: Some(Box::new(source)),
        })
    }
}

pub fn load() -> Result<AppConfig, AppError> {
    AppConfig::load()
}
