use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use pa_core::{AppConfig, AppError};

#[test]
fn load_from_path_reads_expected_fields() {
    let temp_dir = create_temp_dir("valid");
    let config_path = temp_dir.join("config.toml");

    fs::write(
        &config_path,
        r#"
database_url = "sqlite::memory:"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://eastmoney.example"
twelvedata_base_url = "https://twelvedata.example"
twelvedata_api_key = "secret"

[llm.providers.default]
base_url = "https://api.example.com"
api_key = "secret-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.default]
provider = "default"
model = "demo-model"
max_tokens = 1000
max_retries = 1
per_call_timeout_secs = 30
retry_initial_backoff_ms = 100
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.default]
execution_profile = "default"
"#,
    )
    .expect("config should be written");

    let config = AppConfig::load_from_path(&config_path).expect("config should parse");

    assert_eq!(config.database_url, "sqlite::memory:");
    assert_eq!(config.server_addr, "127.0.0.1:3000");
    assert_eq!(config.eastmoney_base_url, "https://eastmoney.example");
    assert_eq!(config.twelvedata_base_url, "https://twelvedata.example");
    assert_eq!(config.twelvedata_api_key, "secret");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_rejects_unknown_keys_and_preserves_parse_source() {
    let temp_dir = create_temp_dir("unknown-key");
    let config_path = temp_dir.join("config.toml");

    fs::write(
        &config_path,
        r#"
database_url = "sqlite::memory:"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://eastmoney.example"
twelvedata_base_url = "https://twelvedata.example"
twelvedata_api_key = "secret"

[llm.providers.default]
base_url = "https://api.example.com"
api_key = "secret-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.default]
provider = "default"
model = "demo-model"
max_tokens = 1000
max_retries = 1
per_call_timeout_secs = 30
retry_initial_backoff_ms = 100
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.default]
execution_profile = "default"
unexpected = "boom"
"#,
    )
    .expect("config should be written");

    let error = AppConfig::load_from_path(&config_path).expect_err("unknown keys should fail");

    match error {
        AppError::Validation { .. } => {}
        other => panic!("expected validation error, got {other}"),
    }

    assert!(
        Error::source(&error).is_some(),
        "expected underlying parse source"
    );

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_reads_llm_provider_profiles_and_bindings() {
    let path = std::env::temp_dir().join(format!(
        "llm-config-{}.toml",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    fs::write(
        &path,
        r#"
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo"

[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "deepseek-key"
openai_api_style = "chat_completions"

[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "dashscope-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.shared_bar_reasoner]
provider = "deepseek"
model = "deepseek-reasoner"
max_tokens = 32768
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen-plus"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"
"#,
    )
    .expect("config should be written");

    let config = AppConfig::load_from_path(&path).expect("config should parse");

    assert_eq!(config.llm.providers.len(), 2);
    assert_eq!(
        config.llm.providers["deepseek"].base_url,
        "https://api.deepseek.com"
    );
    assert_eq!(
        config.llm.execution_profiles["shared_bar_reasoner"].model,
        "deepseek-reasoner"
    );
    assert_eq!(
        config.llm.step_bindings["shared_pa_state_bar_v1"].execution_profile,
        "pa_state_extract_fast"
    );

    fs::remove_file(&path).expect("temp file should be removed");
}

#[test]
fn load_from_path_rejects_execution_profile_provider_that_is_missing() {
    let temp_dir = create_temp_dir("missing-provider");
    let config_path = temp_dir.join("config.toml");

    let config = valid_llm_config_toml().replace(
        r#"provider = "default""#,
        r#"provider = "missing-provider""#,
    );
    fs::write(&config_path, config).expect("config should be written");

    let error = AppConfig::load_from_path(&config_path)
        .expect_err("execution profile provider must reference existing provider");

    match error {
        AppError::Validation { message, source } => {
            assert!(message.contains("execution_profiles.default.provider"));
            assert!(message.contains("missing-provider"));
            assert!(source.is_none());
        }
        other => panic!("expected validation error, got {other}"),
    }

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_rejects_step_binding_execution_profile_that_is_missing() {
    let temp_dir = create_temp_dir("missing-execution-profile");
    let config_path = temp_dir.join("config.toml");

    let config = valid_llm_config_toml().replace(
        r#"execution_profile = "default""#,
        r#"execution_profile = "missing-profile""#,
    );
    fs::write(&config_path, config).expect("config should be written");

    let error = AppConfig::load_from_path(&config_path)
        .expect_err("step binding should reference existing execution profile");

    match error {
        AppError::Validation { message, source } => {
            assert!(message.contains("step_bindings.default.execution_profile"));
            assert!(message.contains("missing-profile"));
            assert!(source.is_none());
        }
        other => panic!("expected validation error, got {other}"),
    }

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_rejects_unknown_openai_api_style() {
    let temp_dir = create_temp_dir("invalid-openai-api-style");
    let config_path = temp_dir.join("config.toml");

    let config = valid_llm_config_toml().replace(
        r#"openai_api_style = "chat_completions""#,
        r#"openai_api_style = "legacy_completions""#,
    );
    fs::write(&config_path, config).expect("config should be written");

    let error =
        AppConfig::load_from_path(&config_path).expect_err("invalid api style should be rejected");

    match error {
        AppError::Validation { source, .. } => {
            let parse_source = source.expect("parse source should exist");
            let details = parse_source.to_string();
            assert!(details.contains("openai_api_style"));
            assert!(details.contains("legacy_completions"));
        }
        other => panic!("expected validation error, got {other}"),
    }

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_parses_config_example_toml() {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("config.example.toml");

    let config = AppConfig::load_from_path(&config_path).expect("example config should parse");

    assert!(config.llm.providers.contains_key("deepseek"));
    assert!(config.llm.providers.contains_key("dashscope"));
    assert!(
        config
            .llm
            .execution_profiles
            .contains_key("pa_state_extract_fast")
    );
    assert!(
        config
            .llm
            .step_bindings
            .contains_key("shared_pa_state_bar_v1")
    );
}

fn create_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "pa-core-config-tests-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn cleanup_temp_dir(path: &Path) {
    fs::remove_dir_all(path).expect("temp dir should be removed");
}

fn valid_llm_config_toml() -> String {
    r#"
database_url = "sqlite::memory:"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://eastmoney.example"
twelvedata_base_url = "https://twelvedata.example"
twelvedata_api_key = "secret"

[llm.providers.default]
base_url = "https://api.example.com"
api_key = "secret-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.default]
provider = "default"
model = "demo-model"
max_tokens = 1000
max_retries = 1
per_call_timeout_secs = 30
retry_initial_backoff_ms = 100
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.default]
execution_profile = "default"
"#
    .to_string()
}
