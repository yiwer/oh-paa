use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use pa_core::config::OpenAiApiStyle;

static TEMP_CONFIG_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn load_replay_config_keeps_current_oh_paa_shape_unchanged() {
    let path = write_temp_toml(
        "current-shape",
        r#"
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
bootstrap_local_test_instruments = false
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm.providers.default]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
openai_api_style = "chat_completions"

[llm.execution_profiles.default]
provider = "default"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "default"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "default"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "default"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "default"
"#,
    );

    let expected = pa_core::AppConfig::load_from_path(&path).expect("current shape should parse");
    let resolved = pa_app::replay_config::load_replay_config(&path).expect("loader should pass");

    assert_eq!(resolved.source_path, path);
    assert_eq!(resolved.app_config, expected);
}

#[test]
fn load_replay_config_normalizes_pa_analyze_server_shape() {
    let path = write_temp_toml(
        "pa-analyze-shape",
        r#"
[proxy]
url = "http://127.0.0.1:10808"

[server]
port = 3000

[database]
url = "postgres://postgres:pgsql@localhost:5432/pa_analyze"
max_connections = 10

[providers.twelvedata]
base_url = "https://api.twelvedata.com"
api_key = "demo-key"
ws_url = "wss://ws.twelvedata.com/v1/quotes/price"

[llm.qwen]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "dashscope-demo"
model = "deepseek-v3.2"
max_tokens = 32765
"#,
    );

    let resolved = pa_app::replay_config::load_replay_config(&path).expect("loader should pass");

    assert_eq!(
        resolved.app_config.database_url,
        "postgres://postgres:pgsql@localhost:5432/oh_paa"
    );
    assert_eq!(resolved.app_config.server_addr, "127.0.0.1:3000");
    assert_eq!(
        resolved.app_config.eastmoney_base_url,
        "https://push2his.eastmoney.com/"
    );
    assert_eq!(
        resolved.app_config.twelvedata_base_url,
        "https://api.twelvedata.com"
    );
    assert_eq!(resolved.app_config.twelvedata_api_key, "demo-key");
    assert_eq!(
        resolved.app_config.llm.providers["qwen"].base_url,
        "https://dashscope.aliyuncs.com/compatible-mode/v1"
    );
    assert_eq!(
        resolved.app_config.llm.providers["qwen"].openai_api_style,
        OpenAiApiStyle::ChatCompletions
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].provider,
        "qwen"
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].model,
        "deepseek-v3.2"
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].max_retries,
        2
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].per_call_timeout_secs,
        600
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].retry_initial_backoff_ms,
        1000
    );

    assert_baseline_steps_bound_to(
        &resolved.app_config,
        "baseline_a_default",
        "pa-analyze-server shape",
    );
}

#[test]
fn load_replay_config_rejects_pa_analyze_server_shape_with_multiple_llm_providers() {
    let path = write_temp_toml(
        "pa-analyze-multi-provider-shape",
        r#"
[providers.twelvedata]
base_url = "https://api.twelvedata.com/"
api_key = "demo-key"

[llm.qwen]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "dashscope-demo"
model = "deepseek-v3.2"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000

[llm.deepseek]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
"#,
    );

    let error = pa_app::replay_config::load_replay_config(&path)
        .expect_err("multiple legacy llm providers should be rejected");
    let message = error.to_string();

    assert!(message.contains("exactly one"));
    assert!(message.contains("llm.<provider>"));
    assert!(message.contains("qwen"));
    assert!(message.contains("deepseek"));
}

#[test]
fn load_replay_config_normalizes_stock_everyday_shape() {
    let path = write_temp_toml(
        "stock-everyday-shape",
        r#"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
"#,
    );

    let resolved = pa_app::replay_config::load_replay_config(&path).expect("loader should pass");

    assert_eq!(
        resolved.app_config.database_url,
        "postgres://postgres:pgsql@localhost:5432/oh_paa"
    );
    assert_eq!(resolved.app_config.server_addr, "127.0.0.1:3000");
    assert_eq!(
        resolved.app_config.eastmoney_base_url,
        "https://push2his.eastmoney.com/"
    );
    assert_eq!(
        resolved.app_config.twelvedata_base_url,
        "https://api.twelvedata.com/"
    );
    assert_eq!(resolved.app_config.twelvedata_api_key, "demo-key");
    assert_eq!(
        resolved.app_config.llm.providers["default"].base_url,
        "https://api.deepseek.com"
    );
    assert_eq!(
        resolved.app_config.llm.providers["default"].openai_api_style,
        OpenAiApiStyle::ChatCompletions
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].provider,
        "default"
    );
    assert_eq!(
        resolved.app_config.llm.execution_profiles["baseline_a_default"].model,
        "deepseek-v4-pro"
    );

    assert_baseline_steps_bound_to(
        &resolved.app_config,
        "baseline_a_default",
        "stock-everyday shape",
    );
}

#[test]
fn load_replay_config_legacy_normalization_remains_compatible_with_runtime_step_registry() {
    let path = write_temp_toml(
        "stock-everyday-runtime-registry",
        r#"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
"#,
    );

    let resolved = pa_app::replay_config::load_replay_config(&path).expect("loader should pass");
    let registry = pa_app::build_step_registry_from_config(&resolved.app_config)
        .expect("normalized legacy config should satisfy runtime step requirements");

    assert!(registry.resolve("shared_pa_state_bar", "v1").is_some());
    assert!(registry.resolve("shared_bar_analysis", "v2").is_some());
    assert!(registry.resolve("shared_daily_context", "v2").is_some());
    assert!(registry.resolve("user_position_advice", "v2").is_some());
}

#[test]
fn load_replay_config_keeps_bootstrap_flag_disabled_for_current_shape() {
    let path = write_temp_toml(
        "current-shape-bootstrap-flag",
        r#"
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
bootstrap_local_test_instruments = false
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm.providers.default]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
openai_api_style = "chat_completions"

[llm.execution_profiles.default]
provider = "default"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "default"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "default"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "default"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "default"
"#,
    );

    let resolved = pa_app::replay_config::load_replay_config(&path).expect("loader should pass");

    assert_eq!(resolved.app_config.bootstrap_local_test_instruments, false);
}

#[test]
fn load_replay_config_preserves_current_shape_parse_details_when_legacy_fallback_fails() {
    let path = write_temp_toml(
        "invalid-current-shape",
        r#"
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
bootstrap_local_test_instruments = false
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "demo-key"

[llm.providers.default]
base_url = "https://api.deepseek.com"
api_key = "deepseek-demo"
openai_api_style = "legacy_completions"

[llm.execution_profiles.default]
provider = "default"
model = "deepseek-v4-pro"
max_tokens = 32765
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "default"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "default"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "default"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "default"
"#,
    );

    let error = pa_app::replay_config::load_replay_config(&path)
        .expect_err("invalid current shape should not be silently replaced");
    let message = error.to_string();

    assert!(message.contains("current-shape"));
    assert!(message.contains("openai_api_style"));
    assert!(message.contains("legacy_completions"));
}

fn assert_baseline_steps_bound_to(config: &pa_core::AppConfig, profile: &str, context: &str) {
    for binding in [
        "shared_pa_state_bar_v1",
        "shared_bar_analysis_v2",
        "shared_daily_context_v2",
        "user_position_advice_v2",
    ] {
        assert_eq!(
            config.llm.step_bindings[binding].execution_profile, profile,
            "{context}: {binding} should map to baseline profile"
        );
    }
}

fn write_temp_toml(label: &str, raw: &str) -> std::path::PathBuf {
    let sequence = TEMP_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "pa-app-replay-config-{label}-{}-{}-{sequence}.toml",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    fs::write(&path, raw).expect("temp config should be written");
    path
}
