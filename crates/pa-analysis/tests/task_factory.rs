use chrono::{NaiveDate, TimeZone, Utc};
use pa_analysis::{
    build_shared_bar_analysis_task, build_shared_daily_context_task, shared_bar_analysis_v1,
    shared_daily_context_v1, SharedBarAnalysisInput, SharedDailyContextInput,
};
use pa_core::Timeframe;
use pa_orchestrator::{build_shared_bar_dedupe_key, sha256_json, AnalysisBarState};
use serde_json::Value;
use uuid::Uuid;

#[test]
fn closed_shared_bar_task_has_dedupe_key_and_open_shared_bar_task_does_not() {
    let instrument_id = Uuid::new_v4();
    let bar_open_time = Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap();
    let bar_close_time = Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap();
    let input = SharedBarAnalysisInput {
        instrument_id,
        timeframe: Timeframe::M15,
        bar_open_time,
        bar_close_time,
        bar_state: AnalysisBarState::Closed,
        canonical_bar_json: serde_json::json!({"close": 101.0}),
        structure_context_json: serde_json::json!({"recent_bars": []}),
    };

    let closed = build_shared_bar_analysis_task(input.clone()).unwrap();
    let expected_input_json = serde_json::to_value(&input).unwrap();
    let open = build_shared_bar_analysis_task(SharedBarAnalysisInput {
        bar_state: AnalysisBarState::Open,
        ..input
    })
    .unwrap();

    let expected_closed_key = build_shared_bar_dedupe_key(
        instrument_id,
        Timeframe::M15,
        bar_close_time,
        &shared_bar_analysis_v1().prompt_key,
        &shared_bar_analysis_v1().prompt_version,
        AnalysisBarState::Closed,
    );

    assert_eq!(
        closed.snapshot.schema_version,
        shared_bar_analysis_v1().input_schema_version
    );
    assert_eq!(closed.snapshot.input_json, expected_input_json);
    assert_eq!(closed.task.prompt_key, shared_bar_analysis_v1().prompt_key);
    assert_eq!(closed.task.prompt_version, shared_bar_analysis_v1().prompt_version);
    assert_eq!(closed.task.dedupe_key, expected_closed_key);
    assert_eq!(open.task.dedupe_key, None);
}

#[test]
fn shared_daily_context_task_snapshot_captures_required_pa_inputs() {
    let input = SharedDailyContextInput {
        instrument_id: Uuid::new_v4(),
        trading_date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
        m15_structure_json: serde_json::json!({"swing_points": []}),
        h1_structure_json: serde_json::json!({"swing_points": []}),
        d1_structure_json: serde_json::json!({"swing_points": []}),
        recent_shared_bar_analyses_json: serde_json::json!([]),
        key_levels_json: serde_json::json!({"support": [], "resistance": []}),
        signal_bar_candidates_json: serde_json::json!([]),
        market_background_json: serde_json::json!({"session": "asia"}),
    };

    let envelope = build_shared_daily_context_task(input.clone()).unwrap();
    assert_eq!(envelope.task.task_type, "shared_daily_context");
    assert_eq!(envelope.task.prompt_key, "shared_daily_context");
    assert_eq!(envelope.task.prompt_version, "v1");
    assert_eq!(envelope.task.bar_state, AnalysisBarState::None);
    assert_eq!(envelope.task.timeframe, None);
    assert_eq!(envelope.task.trading_date, Some(input.trading_date));
    assert!(envelope.task.dedupe_key.is_some());
    assert_eq!(
        envelope.snapshot.schema_version,
        shared_daily_context_v1().input_schema_version
    );
    assert_eq!(envelope.task.prompt_key, shared_daily_context_v1().prompt_key);
    assert_eq!(
        envelope.task.prompt_version,
        shared_daily_context_v1().prompt_version
    );

    assert_eq!(
        envelope.snapshot.input_json,
        serde_json::to_value(&input).unwrap()
    );

    assert_eq!(
        envelope.snapshot.input_hash,
        sha256_json(&envelope.snapshot.input_json).unwrap()
    );
}

#[test]
fn shared_prompt_specs_include_required_pa_contract_fields() {
    let bar_spec = shared_bar_analysis_v1();
    let bar_required = required_fields(&bar_spec.output_json_schema);

    for field in [
        "bar_state",
        "bar_classification",
        "bullish_case",
        "bearish_case",
        "two_sided_summary",
        "nearby_levels",
        "signal_strength",
        "continuation_scenarios",
        "reversal_scenarios",
        "invalidation_levels",
        "execution_bias_notes",
    ] {
        assert!(bar_required.contains(&field.to_string()));
    }

    let daily_spec = shared_daily_context_v1();
    let daily_required = required_fields(&daily_spec.output_json_schema);

    for field in [
        "market_background",
        "market_structure",
        "key_support_levels",
        "key_resistance_levels",
        "signal_bars",
        "candle_patterns",
        "decision_tree_nodes",
        "liquidity_context",
        "risk_notes",
        "scenario_map",
    ] {
        assert!(daily_required.contains(&field.to_string()));
    }

    let decision_required = required_fields(
        &daily_spec.output_json_schema["properties"]["decision_tree_nodes"],
    );
    for field in [
        "trend_context",
        "location_context",
        "signal_quality",
        "confirmation_state",
        "invalidation_conditions",
    ] {
        assert!(decision_required.contains(&field.to_string()));
    }
}

fn required_fields(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|fields| {
            fields
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
