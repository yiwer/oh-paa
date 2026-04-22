use std::{
    collections::HashMap,
    fs,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use pa_app::{
    replay::{ReplayExecutionMode, ReplayStepRun},
    replay_config::load_replay_config,
    replay_live::{
        LiveReplayDataset, LiveReplayExecutor, load_live_replay_dataset,
        run_live_historical_replay_from_path, run_live_replay_with_dependencies,
    },
};
use pa_core::AppError;
use pa_market::{ProviderRouter, provider::providers::TwelveDataProvider};
use pa_orchestrator::{ExecutionAttempt, ExecutionOutcome};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::types::{
    Decimal,
    chrono::{DateTime, Utc},
};

static TEMP_DATASET_COUNTER: AtomicU64 = AtomicU64::new(0);
static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn score_step_runs_reports_completeness_and_consistency_metrics() {
    let scores = pa_app::replay_score::score_step_runs(&sample_live_step_runs());

    assert!(scores.contains_key("decision_tree_completeness"));
    assert!(scores.contains_key("key_level_completeness"));
    assert!(scores.contains_key("signal_bar_completeness"));
    assert!(scores.contains_key("bull_bear_dual_path_completeness"));
    assert!(scores.contains_key("cross_step_consistency_rate"));
}

#[test]
fn replay_cli_parser_requires_config_for_live_mode() {
    let error = pa_app::replay::parse_replay_cli_args([
        "replay_analysis",
        "--mode",
        "live",
        "--dataset",
        "testdata/analysis_replay/live_crypto_15m.json",
        "--variant",
        "baseline_a",
    ])
    .expect_err("live mode without config must fail");

    assert!(error.to_string().contains("--config"));
}

#[test]
fn replay_cli_parser_rejects_missing_dataset_value() {
    let error = pa_app::replay::parse_replay_cli_args([
        "replay_analysis",
        "--dataset",
        "--variant",
        "baseline_a",
    ])
    .expect_err("missing --dataset value must fail");

    assert!(error.to_string().contains("--dataset"));
}

#[test]
fn replay_cli_parser_rejects_missing_mode_value() {
    let error = pa_app::replay::parse_replay_cli_args([
        "replay_analysis",
        "--mode",
        "--dataset",
        "testdata/analysis_replay/live_crypto_15m.json",
        "--variant",
        "baseline_a",
    ])
    .expect_err("missing --mode value must fail");

    assert!(error.to_string().contains("--mode"));
}

#[test]
fn score_step_runs_penalizes_schema_invalid_runs_in_completeness_metrics() {
    let mut step_runs = sample_live_step_runs();
    step_runs.push(ReplayStepRun {
        sample_id: "sample-2".to_string(),
        market: "crypto".to_string(),
        timeframe: "15m".to_string(),
        step_key: "shared_pa_state_bar".to_string(),
        step_version: "v1".to_string(),
        prompt_version: "v1".to_string(),
        llm_provider: "dashscope".to_string(),
        model: "deepseek-v3.2".to_string(),
        input_json: json!({}),
        output_json: json!({}),
        raw_response_json: Some(json!({ "ok": false })),
        schema_valid: false,
        schema_validation_error: Some("missing required fields".to_string()),
        failure_category: Some("schema_validation_failure".to_string()),
        outbound_error_message: None,
        latency_ms: Some(180),
        judge_score: None,
        human_notes: None,
    });

    let scores = pa_app::replay_score::score_step_runs(&step_runs);

    assert_eq!(scores["decision_tree_completeness"].as_f64(), Some(0.5));
}

#[test]
fn score_step_runs_penalizes_samples_missing_a_complete_cross_step_chain() {
    let scores = pa_app::replay_score::score_step_runs(&sample_cross_step_runs());

    assert_eq!(scores["cross_step_consistency_rate"].as_f64(), Some(0.5));
}

#[test]
fn live_replay_dataset_loads_and_matches_first_slice_contract() {
    let _guard = process_test_lock();
    let dataset = load_live_replay_dataset("testdata/analysis_replay/live_crypto_15m.json")
        .expect("live replay dataset should load");

    assert_eq!(dataset.dataset_id, "live_crypto_15m_v1");
    assert_eq!(dataset.market, "crypto");
    assert_eq!(dataset.timeframe, "15m");
    assert_eq!(dataset.pipeline_variant, "baseline_a");
    assert_eq!(dataset.samples.len(), 5);

    let first_sample = &dataset.samples[0];
    assert_eq!(first_sample.provider, "twelvedata");
    assert_eq!(first_sample.provider_symbol, "BTC/USD");
    assert_eq!(
        first_sample.instrument_id.to_string(),
        "22222222-2222-2222-2222-222222222202"
    );
    assert_eq!(
        first_sample.target_bar_open_time.to_rfc3339(),
        "2026-04-18T08:00:00+00:00"
    );
    assert_eq!(
        first_sample.target_bar_close_time.to_rfc3339(),
        "2026-04-18T08:15:00+00:00"
    );
    assert_eq!(first_sample.lookback_15m_bar_count, 12);
    assert_eq!(first_sample.warmup_bar_count, 8);
    assert!(first_sample.user_subscription_json.is_object());
    assert!(first_sample.user_position_json.is_object());
}

#[test]
fn live_replay_dataset_rejects_too_small_warmup_count() {
    let _guard = process_test_lock();
    let dataset_path = pa_app::workspace_root()
        .join("testdata")
        .join("analysis_replay")
        .join("live_crypto_15m.json");
    let dataset_raw = fs::read_to_string(dataset_path).unwrap();
    let mut dataset_json: serde_json::Value = serde_json::from_str(&dataset_raw).unwrap();
    dataset_json["samples"][0]["warmup_bar_count"] = json!(7);
    let path = write_temp_dataset_json(&dataset_json);

    let error = load_live_replay_dataset(&path).expect_err("warmup < 8 should fail validation");

    let message = error.to_string();
    assert!(message.contains("warmup_bar_count"));
    assert!(message.contains("at least 8"));
}

#[test]
fn live_replay_dataset_rejects_impossible_warmup_vs_lookback_depth() {
    let _guard = process_test_lock();
    let mut dataset_json = load_fixture_dataset_json();
    dataset_json["samples"][0]["lookback_15m_bar_count"] = json!(7);
    dataset_json["samples"][0]["warmup_bar_count"] = json!(8);
    let path = write_temp_dataset_json(&dataset_json);

    let error = load_live_replay_dataset(&path)
        .expect_err("lookback depth below warmup must fail validation");

    let message = error.to_string();
    assert!(message.contains("lookback_15m_bar_count"));
    assert!(message.contains("warmup_bar_count"));
}

#[test]
fn live_replay_dataset_rejects_lookback_equal_to_warmup_count() {
    let _guard = process_test_lock();
    let mut dataset_json = load_fixture_dataset_json();
    dataset_json["samples"][0]["lookback_15m_bar_count"] = json!(8);
    dataset_json["samples"][0]["warmup_bar_count"] = json!(8);
    let path = write_temp_dataset_json(&dataset_json);

    let error = load_live_replay_dataset(&path)
        .expect_err("lookback equal to warmup must fail because target bar is required");

    let message = error.to_string();
    assert!(message.contains("lookback_15m_bar_count"));
    assert!(message.contains("warmup_bar_count"));
}

#[test]
fn live_replay_dataset_rejects_duplicate_sample_id() {
    let _guard = process_test_lock();
    let mut dataset_json = load_fixture_dataset_json();
    dataset_json["samples"][1]["sample_id"] = dataset_json["samples"][0]["sample_id"].clone();
    let path = write_temp_dataset_json(&dataset_json);

    let error = load_live_replay_dataset(&path).expect_err("duplicate sample_id must fail");

    let message = error.to_string();
    assert!(message.contains("duplicate sample_id"));
}

#[test]
fn live_replay_dataset_rejects_out_of_order_target_time() {
    let _guard = process_test_lock();
    let mut dataset_json = load_fixture_dataset_json();
    dataset_json["samples"][2]["target_bar_open_time"] = json!("2026-04-18T08:00:00Z");
    dataset_json["samples"][2]["target_bar_close_time"] = json!("2026-04-18T08:15:00Z");
    let path = write_temp_dataset_json(&dataset_json);

    let error = load_live_replay_dataset(&path).expect_err("out-of-order target times should fail");

    let message = error.to_string();
    assert!(message.contains("target bar"));
    assert!(message.contains("order"));
}

#[tokio::test]
async fn live_replay_runner_builds_warmup_context_and_executes_target_chain() {
    let _guard = process_test_lock();
    let dataset = single_sample_dataset();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let sample = dataset.samples[0].clone();
    let server = test_server_for_values(build_twelvedata_values_json(
        utc("2026-04-18T05:15:00Z"),
        12,
    ))
    .await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(build_success_outcomes(sample.warmup_bar_count));

    let report =
        run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
            .await
            .expect("live replay should succeed with injected dependencies");

    assert_eq!(report.execution_mode, ReplayExecutionMode::LiveHistorical);
    assert_eq!(
        report.config_source_path,
        Some(resolved_config.source_path.display().to_string())
    );
    assert_eq!(report.step_runs.len(), 4);
    assert_eq!(
        target_step_keys(&report.step_runs),
        vec![
            "shared_pa_state_bar",
            "shared_bar_analysis",
            "shared_daily_context",
            "user_position_advice",
        ]
    );
    assert!(
        report
            .step_runs
            .iter()
            .all(|run| run.raw_response_json.is_some())
    );
    assert!(report.step_runs.iter().all(|run| run.latency_ms.is_some()));

    let requests = executor.requests();
    assert_eq!(requests.len(), sample.warmup_bar_count * 2 + 4);

    let target_pa_state_input = &requests[sample.warmup_bar_count * 2].input_json;
    assert_eq!(
        target_pa_state_input["bar_json"]["kind"],
        json!("canonical_closed_bar")
    );
    assert_eq!(
        target_pa_state_input["bar_json"]["open_time"],
        json!(sample.target_bar_open_time.to_rfc3339())
    );
    assert_eq!(
        target_pa_state_input["market_context_json"]["multi_timeframe_structure"]["1h"][0]["timeframe"],
        json!("1h")
    );
    assert_eq!(
        target_pa_state_input["market_context_json"]["multi_timeframe_structure"]["1d"][0]["timeframe"],
        json!("1d")
    );

    let target_bar_input = &requests[sample.warmup_bar_count * 2 + 1].input_json;
    assert_eq!(
        target_bar_input["recent_pa_states_json"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    assert_eq!(
        target_bar_input["recent_pa_states_json"][0]["decision_tree_state"]["tag"],
        json!("warmup-pa-0")
    );
    assert_eq!(
        target_bar_input["recent_pa_states_json"][8]["decision_tree_state"]["tag"],
        json!("target-pa")
    );
    assert_eq!(
        target_bar_input["shared_pa_state_json"]["decision_tree_state"]["tag"],
        json!("target-pa")
    );

    let target_daily_input = &requests[sample.warmup_bar_count * 2 + 2].input_json;
    assert_eq!(
        target_daily_input["recent_pa_states_json"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    assert_eq!(
        target_daily_input["recent_pa_states_json"][0]["decision_tree_state"]["tag"],
        json!("warmup-pa-0")
    );
    assert_eq!(
        target_daily_input["recent_pa_states_json"][8]["decision_tree_state"]["tag"],
        json!("target-pa")
    );
    assert_eq!(
        target_daily_input["recent_shared_bar_analyses_json"][0]["market_story"]["tag"],
        json!("warmup-bar-0")
    );
    assert_eq!(
        target_daily_input["recent_shared_bar_analyses_json"][8]["market_story"]["tag"],
        json!("target-bar")
    );

    let target_user_input = &requests[sample.warmup_bar_count * 2 + 3].input_json;
    assert_eq!(
        target_user_input["shared_bar_analysis_json"]["market_story"]["tag"],
        json!("target-bar")
    );
    assert_eq!(
        target_user_input["shared_daily_context_json"]["market_background"]["tag"],
        json!("target-daily")
    );
    assert_eq!(
        target_user_input["shared_pa_state_json"]["decision_tree_state"]["tag"],
        json!("target-pa")
    );

    let observed_requests = server.requests();
    assert_eq!(observed_requests.len(), 1);
    let request = &observed_requests[0];
    assert_eq!(request.query_value("symbol"), Some("BTC/USD"));
    assert_eq!(request.query_value("interval"), Some("15min"));
    assert_eq!(request.query_value("order"), Some("asc"));
    assert_eq!(
        request.query_value("start_date"),
        Some("2026-04-18T05:15:00+00:00")
    );
    assert_eq!(
        request.query_value("end_date"),
        Some("2026-04-18T08:15:00+00:00")
    );
}

#[tokio::test]
async fn live_replay_runner_converts_outbound_failures_into_report_fields() {
    let _guard = process_test_lock();
    let dataset = single_sample_dataset();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let sample = dataset.samples[0].clone();
    let server = test_server_for_values(build_twelvedata_values_json(
        utc("2026-04-18T05:15:00Z"),
        12,
    ))
    .await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(build_failure_outcomes(sample.warmup_bar_count));

    let report =
        run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
            .await
            .expect("live replay should record target failure");

    assert_eq!(report.execution_mode, ReplayExecutionMode::LiveHistorical);
    assert_eq!(report.step_runs.len(), 4);

    let failed = report
        .step_runs
        .iter()
        .find(|run| run.step_key == "user_position_advice")
        .expect("user position advice step should be recorded");
    assert!(!failed.schema_valid);
    assert_eq!(failed.raw_response_json, None);
    assert_eq!(failed.failure_category.as_deref(), Some("outbound_failure"));
    assert!(failed.latency_ms.is_some());
    assert!(
        failed
            .outbound_error_message
            .as_deref()
            .is_some_and(|message| message.contains("simulated upstream timeout"))
    );
}

#[tokio::test]
async fn live_replay_runner_rejects_truncated_provider_history_below_configured_lookback() {
    let _guard = process_test_lock();
    let dataset = single_sample_dataset();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let sample = dataset.samples[0].clone();
    let server = test_server_for_values(build_twelvedata_values_json(
        utc("2026-04-18T05:45:00Z"),
        10,
    ))
    .await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(build_success_outcomes(sample.warmup_bar_count));

    let error =
        run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
            .await
            .expect_err("truncated provider history should be rejected");

    let message = error.to_string();
    assert!(message.contains("lookback_15m_bar_count"));
    assert!(message.contains("fetched 10 rows"));
    assert!(message.contains("expected 12"));
}

#[tokio::test]
async fn live_replay_runner_normalizes_provider_bars_before_replay_conversion() {
    let _guard = process_test_lock();
    let dataset = single_sample_dataset();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let sample = dataset.samples[0].clone();
    let mut invalid_values = build_twelvedata_values_json(utc("2026-04-18T05:15:00Z"), 12);
    invalid_values[0]["high"] = json!("83990");
    let server = test_server_for_values(invalid_values).await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(build_success_outcomes(sample.warmup_bar_count));

    let error =
        run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
            .await
            .expect_err("invalid provider OHLC should be rejected through normalization");

    let message = error.to_string();
    assert!(message.contains("high must be greater than or equal to open and close"));
}

#[tokio::test]
async fn live_replay_runner_derives_trading_date_from_target_bar_open_time() {
    let _guard = process_test_lock();
    let mut dataset = single_sample_dataset();
    dataset.samples[0].target_bar_open_time = utc("2026-04-17T23:45:00Z");
    dataset.samples[0].target_bar_close_time = utc("2026-04-18T00:00:00Z");
    let sample = dataset.samples[0].clone();
    let resolved_config =
        load_replay_config(pa_app::workspace_root().join("config.example.toml")).unwrap();
    let server = test_server_for_values(build_twelvedata_values_json(
        utc("2026-04-17T21:00:00Z"),
        12,
    ))
    .await;
    let mut provider_router = ProviderRouter::default();
    provider_router.insert(Arc::new(TwelveDataProvider::new(
        server.base_url(),
        "test-key",
    )));
    let executor = TestExecutor::new(build_success_outcomes(sample.warmup_bar_count));

    let report =
        run_live_replay_with_dependencies(&dataset, &resolved_config, &provider_router, &executor)
            .await
            .expect("cross-midnight crypto sample should replay");

    assert_eq!(report.step_runs.len(), 4);
    let requests = executor.requests();
    let target_daily_input = &requests[sample.warmup_bar_count * 2 + 2].input_json;
    assert_eq!(target_daily_input["trading_date"], json!("2026-04-17"));
    let target_user_input = &requests[sample.warmup_bar_count * 2 + 3].input_json;
    assert_eq!(target_user_input["trading_date"], json!("2026-04-17"));
}

#[tokio::test]
async fn live_replay_public_entrypoint_resolves_relative_config_path_against_workspace() {
    let _guard = process_test_lock();
    let mut dataset_json = load_fixture_dataset_json();
    dataset_json["samples"][0]["provider"] = json!("missing-provider");
    let dataset_path = write_temp_dataset_json(&dataset_json);
    let temp_cwd = write_temp_working_dir();
    let cwd_guard = CurrentDirGuard::change_to(&temp_cwd);

    let error =
        run_live_historical_replay_from_path(&dataset_path, "config.example.toml", "baseline_a")
            .await
            .expect_err(
                "relative config path should resolve, then runner should fail on missing provider",
            );

    drop(cwd_guard);

    let message = error.to_string();
    assert!(message.contains("provider `missing-provider` is not registered"));
}

fn load_fixture_dataset_json() -> serde_json::Value {
    let dataset_path = pa_app::workspace_root()
        .join("testdata")
        .join("analysis_replay")
        .join("live_crypto_15m.json");
    let dataset_raw = fs::read_to_string(&dataset_path).unwrap();
    serde_json::from_str(&dataset_raw).unwrap()
}

fn write_temp_dataset_json(dataset: &serde_json::Value) -> std::path::PathBuf {
    let sequence = TEMP_DATASET_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "pa-app-live-replay-{}-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        sequence
    ));

    fs::write(&path, serde_json::to_vec_pretty(dataset).unwrap()).unwrap();
    path
}

fn single_sample_dataset() -> LiveReplayDataset {
    let mut dataset = load_live_replay_dataset("testdata/analysis_replay/live_crypto_15m.json")
        .expect("live replay dataset should load");
    dataset.samples.truncate(1);
    dataset
}

fn target_step_keys(step_runs: &[ReplayStepRun]) -> Vec<&str> {
    step_runs.iter().map(|run| run.step_key.as_str()).collect()
}

fn sample_live_step_runs() -> Vec<ReplayStepRun> {
    vec![
        ReplayStepRun {
            sample_id: "sample-1".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_pa_state_bar".to_string(),
            step_version: "v1".to_string(),
            prompt_version: "v1".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "decision_tree_state": {},
                "support_resistance_map": {},
                "signal_assessment": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(200),
            judge_score: None,
            human_notes: None,
        },
        ReplayStepRun {
            sample_id: "sample-1".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v2".to_string(),
            prompt_version: "v2".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "bullish_case": {},
                "bearish_case": {},
                "two_sided_balance": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(250),
            judge_score: None,
            human_notes: None,
        },
    ]
}

fn sample_cross_step_runs() -> Vec<ReplayStepRun> {
    vec![
        ReplayStepRun {
            sample_id: "sample-complete".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_pa_state_bar".to_string(),
            step_version: "v1".to_string(),
            prompt_version: "v1".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "decision_tree_state": {},
                "support_resistance_map": {},
                "signal_assessment": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(200),
            judge_score: None,
            human_notes: None,
        },
        ReplayStepRun {
            sample_id: "sample-complete".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_bar_analysis".to_string(),
            step_version: "v2".to_string(),
            prompt_version: "v2".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "bullish_case": {},
                "bearish_case": {},
                "two_sided_balance": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(220),
            judge_score: None,
            human_notes: None,
        },
        ReplayStepRun {
            sample_id: "sample-complete".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_daily_context".to_string(),
            step_version: "v2".to_string(),
            prompt_version: "v2".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "decision_tree_nodes": {},
                "signal_bars": {},
                "key_support_levels": {},
                "key_resistance_levels": {},
                "candle_pattern_map": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(240),
            judge_score: None,
            human_notes: None,
        },
        ReplayStepRun {
            sample_id: "sample-incomplete".to_string(),
            market: "crypto".to_string(),
            timeframe: "15m".to_string(),
            step_key: "shared_pa_state_bar".to_string(),
            step_version: "v1".to_string(),
            prompt_version: "v1".to_string(),
            llm_provider: "dashscope".to_string(),
            model: "deepseek-v3.2".to_string(),
            input_json: json!({}),
            output_json: json!({
                "decision_tree_state": {},
                "support_resistance_map": {},
                "signal_assessment": {}
            }),
            raw_response_json: Some(json!({ "ok": true })),
            schema_valid: true,
            schema_validation_error: None,
            failure_category: None,
            outbound_error_message: None,
            latency_ms: Some(180),
            judge_score: None,
            human_notes: None,
        },
    ]
}

fn build_success_outcomes(warmup_count: usize) -> Vec<ExecutionOutcome> {
    let mut outcomes = Vec::new();

    for index in 0..warmup_count {
        outcomes.push(success_outcome(shared_pa_state_output(&format!(
            "warmup-pa-{index}"
        ))));
        outcomes.push(success_outcome(shared_bar_output(&format!(
            "warmup-bar-{index}"
        ))));
    }

    outcomes.push(success_outcome(shared_pa_state_output("target-pa")));
    outcomes.push(success_outcome(shared_bar_output("target-bar")));
    outcomes.push(success_outcome(shared_daily_output("target-daily")));
    outcomes.push(success_outcome(user_position_output("target-user")));

    outcomes
}

fn build_failure_outcomes(warmup_count: usize) -> Vec<ExecutionOutcome> {
    let mut outcomes = build_success_outcomes(warmup_count);
    outcomes.pop();
    outcomes.push(outbound_failure_outcome("simulated upstream timeout"));
    outcomes
}

fn success_outcome(output_json: Value) -> ExecutionOutcome {
    ExecutionOutcome::Success(ExecutionAttempt {
        llm_provider: "fixture".to_string(),
        model: "fixture-live".to_string(),
        request_payload_json: Value::Null,
        raw_response_json: Some(output_json.clone()),
        parsed_output_json: Some(output_json),
        schema_validation_error: None,
        outbound_error_message: None,
    })
}

fn outbound_failure_outcome(message: &str) -> ExecutionOutcome {
    ExecutionOutcome::OutboundCallFailed {
        attempt: ExecutionAttempt {
            llm_provider: "fixture".to_string(),
            model: "fixture-live".to_string(),
            request_payload_json: Value::Null,
            raw_response_json: None,
            parsed_output_json: None,
            schema_validation_error: None,
            outbound_error_message: Some(message.to_string()),
        },
        error: AppError::Provider {
            message: message.to_string(),
            source: None,
        },
    }
}

fn shared_pa_state_output(tag: &str) -> Value {
    json!({
        "bar_identity": { "tag": tag },
        "market_session_context": { "tag": tag },
        "bar_observation": { "tag": tag },
        "bar_shape": { "tag": tag },
        "location_context": { "tag": tag },
        "multi_timeframe_alignment": { "tag": tag },
        "support_resistance_map": { "tag": tag },
        "signal_assessment": { "tag": tag },
        "decision_tree_state": { "tag": tag },
        "evidence_log": { "tag": tag }
    })
}

fn shared_bar_output(tag: &str) -> Value {
    json!({
        "bar_identity": { "tag": tag },
        "bar_summary": { "tag": tag },
        "market_story": { "tag": tag },
        "bullish_case": { "tag": tag },
        "bearish_case": { "tag": tag },
        "two_sided_balance": { "tag": tag },
        "key_levels": { "tag": tag },
        "signal_bar_verdict": { "tag": tag },
        "continuation_path": { "tag": tag },
        "reversal_path": { "tag": tag },
        "invalidation_map": { "tag": tag },
        "follow_through_checkpoints": { "tag": tag }
    })
}

fn shared_daily_output(tag: &str) -> Value {
    json!({
        "context_identity": { "tag": tag },
        "market_background": { "tag": tag },
        "dominant_structure": { "tag": tag },
        "intraday_vs_higher_timeframe_state": { "tag": tag },
        "key_support_levels": { "tag": tag },
        "key_resistance_levels": { "tag": tag },
        "signal_bars": { "tag": tag },
        "candle_pattern_map": { "tag": tag },
        "decision_tree_nodes": { "tag": tag },
        "liquidity_context": { "tag": tag },
        "scenario_map": { "tag": tag },
        "risk_notes": { "tag": tag },
        "session_playbook": { "tag": tag }
    })
}

fn user_position_output(tag: &str) -> Value {
    json!({
        "position_state": { "tag": tag },
        "market_read_through": { "tag": tag },
        "bullish_path_for_user": { "tag": tag },
        "bearish_path_for_user": { "tag": tag },
        "hold_reduce_exit_conditions": { "tag": tag },
        "risk_control_levels": { "tag": tag },
        "invalidations": { "tag": tag },
        "action_candidates": { "tag": tag }
    })
}

#[derive(Debug, Clone)]
struct RecordedExecution {
    input_json: Value,
}

#[derive(Debug)]
struct TestExecutor {
    outcomes: Mutex<Vec<ExecutionOutcome>>,
    requests: Mutex<Vec<RecordedExecution>>,
}

impl TestExecutor {
    fn new(outcomes: Vec<ExecutionOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into_iter().rev().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<RecordedExecution> {
        self.requests.lock().unwrap().clone()
    }
}

impl LiveReplayExecutor for TestExecutor {
    fn execute_json<'a>(
        &'a self,
        _step_key: &'a str,
        _step_version: &'a str,
        input_json: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<ExecutionOutcome, AppError>> + Send + 'a>> {
        self.requests.lock().unwrap().push(RecordedExecution {
            input_json: input_json.clone(),
        });
        let outcome = self.outcomes.lock().unwrap().pop().unwrap();

        Box::pin(async move { Ok(outcome) })
    }
}

#[derive(Debug, Clone)]
struct ObservedRequest {
    query: Vec<(String, String)>,
}

impl ObservedRequest {
    fn query_value(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(query_key, _)| query_key == key)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct RequestQuery {
    #[serde(flatten)]
    values: HashMap<String, String>,
}

#[derive(Debug)]
struct TestServer {
    base_url: String,
    state: Arc<TestServerState>,
}

#[derive(Debug)]
struct TestServerState {
    requests: Mutex<Vec<ObservedRequest>>,
    response_values: Vec<Value>,
}

impl TestServer {
    async fn spawn(app: Router, state: Arc<TestServerState>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should keep running");
        });

        Self {
            base_url: format!("http://{}", address),
            state,
        }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    fn requests(&self) -> Vec<ObservedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

async fn twelvedata_time_series(
    State(state): State<Arc<TestServerState>>,
    Query(query): Query<RequestQuery>,
) -> impl IntoResponse {
    let mut requests = state.requests.lock().unwrap();
    let mut query_pairs = query.values.into_iter().collect::<Vec<_>>();
    query_pairs.sort_by(|left, right| left.0.cmp(&right.0));
    requests.push(ObservedRequest { query: query_pairs });

    (
        StatusCode::OK,
        axum::Json(json!({
            "status": "ok",
            "values": state.response_values
        })),
    )
}

async fn test_server_for_values(values: Vec<Value>) -> TestServer {
    let state = Arc::new(TestServerState {
        requests: Mutex::new(Vec::new()),
        response_values: values,
    });
    TestServer::spawn(
        Router::new()
            .route("/time_series", get(twelvedata_time_series))
            .with_state(Arc::clone(&state)),
        state,
    )
    .await
}

fn build_twelvedata_values_json(start_open_time: DateTime<Utc>, count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| {
            let open_time = DateTime::<Utc>::from_timestamp(
                start_open_time.timestamp() + (index as i64 * 900),
                0,
            )
            .expect("fixture timestamp should stay valid");
            let base = Decimal::from(84_000_i64 + (index as i64 * 15));
            json!({
                "datetime": open_time.to_rfc3339(),
                "open": base.to_string(),
                "high": (base + Decimal::from(20_i64)).to_string(),
                "low": (base - Decimal::from(10_i64)).to_string(),
                "close": (base + Decimal::from(12_i64)).to_string(),
                "volume": (Decimal::from(1_000_i64) + Decimal::from(index as i64)).to_string(),
            })
        })
        .collect()
}

fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("timestamp should parse")
        .with_timezone(&Utc)
}

fn process_test_lock() -> std::sync::MutexGuard<'static, ()> {
    match PROCESS_TEST_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_temp_working_dir() -> std::path::PathBuf {
    let sequence = TEMP_DATASET_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "pa-app-live-replay-cwd-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        sequence
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

struct CurrentDirGuard {
    previous: std::path::PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &std::path::Path) -> Self {
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        Self { previous }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
    }
}
