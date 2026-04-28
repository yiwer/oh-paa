use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fs,
    path::{Path, PathBuf},
};

use pa_core::{
    AppConfig, AppError,
    config::{
        LlmConfig, LlmExecutionProfileConfig, LlmProviderConfig, LlmStepBindingConfig,
        OpenAiApiStyle,
    },
};

const DEFAULT_DATABASE_URL: &str = "postgres://postgres:pgsql@localhost:5432/oh_paa";
const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:3000";
const DEFAULT_EASTMONEY_BASE_URL: &str = "https://push2his.eastmoney.com/";
const BASELINE_PROFILE_KEY: &str = "baseline_a_default";
const DEFAULT_LEGACY_MAX_RETRIES: u32 = 2;
const DEFAULT_LEGACY_PER_CALL_TIMEOUT_SECS: u64 = 600;
const DEFAULT_LEGACY_RETRY_INITIAL_BACKOFF_MS: u64 = 1_000;

#[derive(Debug, Clone)]
pub struct ResolvedReplayConfig {
    pub source_path: PathBuf,
    pub app_config: AppConfig,
}

pub fn load_replay_config(path: impl AsRef<Path>) -> Result<ResolvedReplayConfig, AppError> {
    let source_path = path.as_ref().to_path_buf();

    match AppConfig::load_from_path(&source_path) {
        Ok(app_config) => Ok(ResolvedReplayConfig {
            source_path,
            app_config,
        }),
        Err(current_shape_error) => {
            let current_shape_details = format_error_chain(&current_shape_error);

            let raw = fs::read_to_string(&source_path).map_err(|source| AppError::Storage {
                message: format!(
                    "failed to read replay config from {}",
                    source_path.display()
                ),
                source: Some(Box::new(source)),
            })?;

            let app_config = parse_pa_analyze_server_config(&raw)
                .or_else(|pa_error| {
                    parse_stock_everyday_config(&raw)
                        .map_err(|stock_error| format!("pa-analyze-server: {pa_error}; stock-everyday: {stock_error}"))
                })
                .map_err(|legacy_details| AppError::Validation {
                    message: format!(
                        "failed to parse replay config from {} as current-shape or supported legacy shape (current-shape error: {}; legacy-shape errors: {legacy_details})",
                        source_path.display(),
                        current_shape_details
                    ),
                    source: Some(Box::new(current_shape_error)),
                })?;

            Ok(ResolvedReplayConfig {
                source_path,
                app_config,
            })
        }
    }
}

fn parse_pa_analyze_server_config(raw: &str) -> Result<AppConfig, String> {
    let root = parse_toml(raw)?;

    let providers = required_table(&root, "providers", "pa-analyze-server")?;
    let twelvedata = required_table(providers, "twelvedata", "pa-analyze-server providers")?;
    let twelvedata_base_url = required_string(twelvedata, "base_url", "providers.twelvedata")?;
    let twelvedata_api_key = required_string(twelvedata, "api_key", "providers.twelvedata")?;

    let llm = required_table(&root, "llm", "pa-analyze-server")?;
    let mut provider_keys = llm.keys().cloned().collect::<Vec<_>>();
    provider_keys.sort();
    if provider_keys.len() != 1 {
        let listed = if provider_keys.is_empty() {
            "none".to_string()
        } else {
            provider_keys.join(", ")
        };
        return Err(format!(
            "pa-analyze-server legacy shape requires exactly one [llm.<provider>] block, found {} ({listed})",
            provider_keys.len()
        ));
    }

    let provider_key = provider_keys[0].clone();
    let llm_provider = required_table(llm, &provider_key, "pa-analyze-server llm provider")?;
    let legacy_profile =
        parse_legacy_llm_profile(&provider_key, llm_provider, &format!("llm.{provider_key}"))?;

    Ok(normalize_legacy_config(
        twelvedata_base_url,
        twelvedata_api_key,
        legacy_profile,
    ))
}

fn parse_stock_everyday_config(raw: &str) -> Result<AppConfig, String> {
    let root = parse_toml(raw)?;

    let twelvedata_base_url = required_string(&root, "twelvedata_base_url", "stock-everyday")?;
    let twelvedata_api_key = required_string(&root, "twelvedata_api_key", "stock-everyday")?;
    let llm = required_table(&root, "llm", "stock-everyday")?;
    let legacy_profile = parse_legacy_llm_profile("default", llm, "llm")?;

    Ok(normalize_legacy_config(
        twelvedata_base_url,
        twelvedata_api_key,
        legacy_profile,
    ))
}

fn normalize_legacy_config(
    twelvedata_base_url: String,
    twelvedata_api_key: String,
    llm: LegacyLlmProfile,
) -> AppConfig {
    AppConfig {
        database_url: DEFAULT_DATABASE_URL.to_string(),
        server_addr: DEFAULT_SERVER_ADDR.to_string(),
        bootstrap_local_test_instruments: false,
        eastmoney_base_url: DEFAULT_EASTMONEY_BASE_URL.to_string(),
        twelvedata_base_url,
        twelvedata_api_key,
        llm: LlmConfig {
            providers: BTreeMap::from([(
                llm.provider_key.clone(),
                LlmProviderConfig {
                    base_url: llm.base_url,
                    api_key: llm.api_key,
                    openai_api_style: OpenAiApiStyle::ChatCompletions,
                },
            )]),
            execution_profiles: BTreeMap::from([(
                BASELINE_PROFILE_KEY.to_string(),
                LlmExecutionProfileConfig {
                    provider: llm.provider_key,
                    model: llm.model,
                    max_tokens: llm.max_tokens,
                    max_retries: llm.max_retries,
                    per_call_timeout_secs: llm.per_call_timeout_secs,
                    retry_initial_backoff_ms: llm.retry_initial_backoff_ms,
                    supports_json_schema: false,
                    supports_reasoning: false,
                },
            )]),
            step_bindings: baseline_a_step_bindings(BASELINE_PROFILE_KEY),
        },
    }
}

fn baseline_a_step_bindings(profile_key: &str) -> BTreeMap<String, LlmStepBindingConfig> {
    BTreeMap::from([
        (
            "shared_pa_state_bar_v1".to_string(),
            LlmStepBindingConfig {
                execution_profile: profile_key.to_string(),
            },
        ),
        (
            "shared_bar_analysis_v2".to_string(),
            LlmStepBindingConfig {
                execution_profile: profile_key.to_string(),
            },
        ),
        (
            "shared_daily_context_v2".to_string(),
            LlmStepBindingConfig {
                execution_profile: profile_key.to_string(),
            },
        ),
        (
            "user_position_advice_v2".to_string(),
            LlmStepBindingConfig {
                execution_profile: profile_key.to_string(),
            },
        ),
    ])
}

#[derive(Debug, Clone)]
struct LegacyLlmProfile {
    provider_key: String,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    max_retries: u32,
    per_call_timeout_secs: u64,
    retry_initial_backoff_ms: u64,
}

fn parse_legacy_llm_profile(
    provider_key: &str,
    table: &toml::value::Table,
    context: &str,
) -> Result<LegacyLlmProfile, String> {
    Ok(LegacyLlmProfile {
        provider_key: provider_key.to_string(),
        base_url: required_string(table, "base_url", context)?,
        api_key: required_string(table, "api_key", context)?,
        model: required_string(table, "model", context)?,
        max_tokens: required_u32(table, "max_tokens", context)?,
        max_retries: optional_u32(table, "max_retries").unwrap_or(DEFAULT_LEGACY_MAX_RETRIES),
        per_call_timeout_secs: optional_u64(table, "per_call_timeout_secs")
            .unwrap_or(DEFAULT_LEGACY_PER_CALL_TIMEOUT_SECS),
        retry_initial_backoff_ms: optional_u64(table, "retry_initial_backoff_ms")
            .unwrap_or(DEFAULT_LEGACY_RETRY_INITIAL_BACKOFF_MS),
    })
}

fn parse_toml(raw: &str) -> Result<toml::value::Table, String> {
    let value: toml::Value = toml::from_str(raw).map_err(|err| format!("invalid TOML: {err}"))?;
    value
        .as_table()
        .cloned()
        .ok_or_else(|| "top-level TOML value must be a table".to_string())
}

fn required_table<'a>(
    table: &'a toml::value::Table,
    key: &str,
    context: &str,
) -> Result<&'a toml::value::Table, String> {
    table
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("missing or invalid table `{key}` in {context}"))
}

fn required_string(table: &toml::value::Table, key: &str, context: &str) -> Result<String, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing or invalid string `{key}` in {context}"))
}

fn required_u32(table: &toml::value::Table, key: &str, context: &str) -> Result<u32, String> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("missing or invalid integer `{key}` in {context}"))?;
    u32::try_from(value)
        .map_err(|_| format!("`{key}` in {context} must be between 0 and {}", u32::MAX))
}

fn optional_u32(table: &toml::value::Table, key: &str) -> Option<u32> {
    let value = table.get(key)?.as_integer()?;
    u32::try_from(value).ok()
}

fn optional_u64(table: &toml::value::Table, key: &str) -> Option<u64> {
    let value = table.get(key)?.as_integer()?;
    u64::try_from(value).ok()
}

fn format_error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(next) = source {
        parts.push(next.to_string());
        source = next.source();
    }
    parts.join(" | caused by: ")
}
