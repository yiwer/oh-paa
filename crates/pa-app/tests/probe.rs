use pa_app::replay_probe::{ProbeResult, parse_probe_cli_args};
use pa_core::Timeframe;
use pa_orchestrator::AnalysisBarState;
use serde_json::json;

#[test]
fn probe_cli_parser_parses_step_and_input_path() {
    let args = parse_probe_cli_args([
        "replay_probe",
        "--config",
        "config.example.toml",
        "--step",
        "shared_pa_state_bar:v1",
        "--input",
        "testdata/analysis_replay/probe_shared_pa_state_input.json",
    ])
    .expect("probe args should parse");

    assert_eq!(args.config_path, "config.example.toml");
    assert_eq!(args.step_key, "shared_pa_state_bar");
    assert_eq!(args.step_version, "v1");
    assert_eq!(
        args.input_path,
        "testdata/analysis_replay/probe_shared_pa_state_input.json"
    );
}

#[test]
fn probe_cli_parser_requires_step_version_separator() {
    let error = parse_probe_cli_args([
        "replay_probe",
        "--config",
        "config.example.toml",
        "--step",
        "shared_pa_state_bar",
        "--input",
        "testdata/analysis_replay/probe_shared_pa_state_input.json",
    ])
    .expect_err("--step should require key:version");

    assert!(error.to_string().contains("missing step version"));
}

#[test]
fn probe_result_serializes_expected_fields() {
    let result = ProbeResult {
        step_key: "shared_pa_state_bar".to_string(),
        step_version: "v1".to_string(),
        llm_provider: "dashscope".to_string(),
        model: "qwen-plus".to_string(),
        schema_valid: false,
        failure_category: Some("schema_validation".to_string()),
        schema_validation_error: Some("missing required property".to_string()),
        outbound_error_message: None,
        output_json: json!({"signal": "hold"}),
        raw_response_json: Some(json!({"raw": "payload"})),
    };

    let encoded = serde_json::to_value(&result).expect("probe result should serialize");
    assert_eq!(encoded["step_key"], "shared_pa_state_bar");
    assert_eq!(encoded["step_version"], "v1");
    assert_eq!(encoded["llm_provider"], "dashscope");
    assert_eq!(encoded["model"], "qwen-plus");
    assert_eq!(encoded["schema_valid"], false);
    assert_eq!(encoded["failure_category"], "schema_validation");
    assert_eq!(
        encoded["schema_validation_error"],
        "missing required property"
    );
    assert_eq!(encoded["outbound_error_message"], serde_json::Value::Null);
    assert_eq!(encoded["output_json"]["signal"], "hold");
    assert_eq!(encoded["raw_response_json"]["raw"], "payload");
}

#[test]
fn probe_fixture_matches_shared_pa_state_input_shape() {
    let path = pa_app::workspace_root().join("testdata/analysis_replay/probe_shared_pa_state_input.json");
    let raw = std::fs::read_to_string(path).expect("probe fixture should exist");
    let input: pa_analysis::SharedPaStateBarInput =
        serde_json::from_str(&raw).expect("probe fixture should deserialize");

    assert_eq!(input.timeframe, Timeframe::M15);
    assert_eq!(input.bar_state, AnalysisBarState::Closed);
    assert!(input.bar_json.is_object());
    assert!(input.market_context_json.is_object());
}
