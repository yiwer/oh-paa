use chrono::{NaiveDate, TimeZone, Utc};
use pa_analysis::{
    SharedBarAnalysisInput, SharedDailyContextInput, SharedPaStateBarInput,
    build_shared_bar_analysis_task, build_shared_daily_context_task,
    build_shared_pa_state_bar_task, shared_bar_analysis_prompt_v2, shared_bar_analysis_v2,
    shared_daily_context_prompt_v2, shared_daily_context_v2, shared_pa_state_bar_prompt_v1,
    shared_pa_state_bar_v1,
};
use pa_core::{AppError, Timeframe};
use pa_orchestrator::{AnalysisBarState, build_shared_bar_dedupe_key, sha256_json};
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
        shared_pa_state_json: serde_json::json!({"bar_identity": {"tag": "shared-pa-state"}}),
        recent_pa_states_json: serde_json::json!([]),
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
        &shared_bar_analysis_v2().step_key,
        &shared_bar_analysis_v2().step_version,
        AnalysisBarState::Closed,
    );

    assert_eq!(
        closed.snapshot.schema_version,
        shared_bar_analysis_v2().input_schema_version
    );
    assert_eq!(closed.snapshot.input_json, expected_input_json);
    assert_eq!(closed.task.prompt_key, shared_bar_analysis_v2().step_key);
    assert_eq!(
        closed.task.prompt_version,
        shared_bar_analysis_v2().step_version
    );
    assert_eq!(closed.task.dedupe_key, expected_closed_key);
    assert_eq!(open.task.dedupe_key, None);
}

#[test]
fn closed_pa_state_task_has_dedupe_key_and_open_task_does_not() {
    let bar_open_time = chrono::DateTime::parse_from_rfc3339("2026-04-21T01:45:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let bar_close_time = chrono::DateTime::parse_from_rfc3339("2026-04-21T02:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let closed = build_shared_pa_state_bar_task(SharedPaStateBarInput {
        instrument_id: Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Closed,
        bar_open_time,
        bar_close_time,
        bar_json: serde_json::json!({"kind":"canonical_closed_bar"}),
        market_context_json: serde_json::json!({"market":{"market_code":"crypto"}}),
    })
    .unwrap();

    let open = build_shared_pa_state_bar_task(SharedPaStateBarInput {
        instrument_id: Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Open,
        bar_open_time,
        bar_close_time,
        bar_json: serde_json::json!({"kind":"derived_open_bar"}),
        market_context_json: serde_json::json!({"market":{"market_code":"crypto"}}),
    })
    .unwrap();

    assert!(closed.task.dedupe_key.is_some());
    assert!(open.task.dedupe_key.is_none());
    assert_eq!(closed.task.task_type, "shared_pa_state_bar");
    assert_eq!(closed.task.prompt_key, "shared_pa_state_bar");
    assert_eq!(closed.task.prompt_version, "v1");
    assert_eq!(
        closed.snapshot.schema_version,
        shared_pa_state_bar_v1().input_schema_version
    );
}

#[test]
fn pa_state_task_rejects_none_bar_state_without_panicking() {
    let error = build_shared_pa_state_bar_task(SharedPaStateBarInput {
        instrument_id: Uuid::nil(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::None,
        bar_open_time: Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap(),
        bar_close_time: Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap(),
        bar_json: serde_json::json!({"kind":"invalid_bar_state"}),
        market_context_json: serde_json::json!({"market":{"market_code":"crypto"}}),
    })
    .unwrap_err();

    match error {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("requires open or closed"));
        }
        other => panic!("expected analysis error, got {other:?}"),
    }
}

#[test]
fn shared_daily_context_task_snapshot_captures_required_pa_inputs() {
    let input = SharedDailyContextInput {
        instrument_id: Uuid::new_v4(),
        trading_date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
        recent_pa_states_json: serde_json::json!([]),
        recent_shared_bar_analyses_json: serde_json::json!([]),
        multi_timeframe_structure_json: serde_json::json!({
            "15m": {"rows": []},
            "1h": {"rows": []},
            "1d": {"rows": []}
        }),
        market_background_json: serde_json::json!({"session": "asia"}),
    };

    let envelope = build_shared_daily_context_task(input.clone()).unwrap();
    assert_eq!(envelope.task.task_type, "shared_daily_context");
    assert_eq!(envelope.task.prompt_key, "shared_daily_context");
    assert_eq!(envelope.task.prompt_version, "v2");
    assert_eq!(envelope.task.bar_state, AnalysisBarState::None);
    assert_eq!(envelope.task.timeframe, None);
    assert_eq!(envelope.task.trading_date, Some(input.trading_date));
    assert!(envelope.task.dedupe_key.is_some());
    assert_eq!(
        envelope.snapshot.schema_version,
        shared_daily_context_v2().input_schema_version
    );
    assert_eq!(envelope.task.prompt_key, shared_daily_context_v2().step_key);
    assert_eq!(
        envelope.task.prompt_version,
        shared_daily_context_v2().step_version
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
    let pa_state_spec = shared_pa_state_bar_v1();
    let pa_state_required = required_fields(&pa_state_spec.output_json_schema);

    for field in [
        "bar_identity",
        "market_session_context",
        "bar_observation",
        "bar_shape",
        "location_context",
        "multi_timeframe_alignment",
        "support_resistance_map",
        "signal_assessment",
        "decision_tree_state",
        "evidence_log",
    ] {
        assert!(pa_state_required.contains(&field.to_string()));
    }

    let pa_state_decision_required =
        required_fields(&pa_state_spec.output_json_schema["properties"]["decision_tree_state"]);
    for field in [
        "trend_context",
        "location_context",
        "signal_quality",
        "confirmation_state",
        "invalidation_conditions",
        "bias_balance",
    ] {
        assert!(pa_state_decision_required.contains(&field.to_string()));
    }

    let bar_spec = shared_bar_analysis_v2();
    let bar_required = required_fields(&bar_spec.output_json_schema);

    for field in [
        "bar_identity",
        "bar_summary",
        "market_story",
        "bullish_case",
        "bearish_case",
        "two_sided_balance",
        "key_levels",
        "signal_bar_verdict",
        "continuation_path",
        "reversal_path",
        "invalidation_map",
        "follow_through_checkpoints",
    ] {
        assert!(bar_required.contains(&field.to_string()));
    }

    let daily_spec = shared_daily_context_v2();
    let daily_required = required_fields(&daily_spec.output_json_schema);
    assert_eq!(
        daily_spec.dependency_policy,
        "requires_shared_pa_state_optional_shared_bar"
    );

    for field in [
        "context_identity",
        "market_background",
        "dominant_structure",
        "intraday_vs_higher_timeframe_state",
        "key_support_levels",
        "key_resistance_levels",
        "signal_bars",
        "candle_pattern_map",
        "decision_tree_nodes",
        "liquidity_context",
        "scenario_map",
        "risk_notes",
        "session_playbook",
    ] {
        assert!(daily_required.contains(&field.to_string()));
    }

    let decision_required =
        required_fields(&daily_spec.output_json_schema["properties"]["decision_tree_nodes"]);
    for field in [
        "trend_context",
        "location_context",
        "signal_quality",
        "confirmation_state",
        "invalidation_conditions",
        "path_of_least_resistance",
    ] {
        assert!(decision_required.contains(&field.to_string()));
    }
}

#[test]
fn shared_pa_state_prompt_v1_requires_complete_decision_tree_and_strict_json() {
    let prompt = shared_pa_state_bar_prompt_v1();
    let instructions = prompt.developer_instructions.join("\n");

    assert!(instructions.contains("decision_tree_state.trend_context"));
    assert!(instructions.contains("decision_tree_state.bias_balance"));
    assert!(instructions.contains("bar_identity"));
    assert!(instructions.contains("evidence_log"));
    assert!(instructions.contains("support_resistance_map"));
    assert!(instructions.contains("signal_assessment"));
    assert!(instructions.contains("Return JSON only"));
}

#[test]
fn shared_pa_state_prompt_v1_emphasizes_no_alias_no_trailing_text_and_object_only_shape() {
    let prompt = shared_pa_state_bar_prompt_v1();
    let instructions = prompt.developer_instructions.join("\n");

    assert!(instructions.contains("Do not use alias keys or near-match keys"));
    assert!(instructions.contains("The first character must be { and the final character must be }"));
    assert!(instructions.contains("Do not include explanatory text before or after the JSON object"));
    assert!(instructions.contains("Every required section must remain a JSON object even when uncertain"));
    assert!(instructions.contains("Keep all reasoning inside structured JSON fields"));
}

#[test]
fn shared_pa_state_schema_v1_rejects_unexpected_top_level_fields() {
    let pa_state_spec = shared_pa_state_bar_v1();
    assert_eq!(
        pa_state_spec.output_json_schema["additionalProperties"],
        serde_json::json!(false)
    );
}

#[test]
fn shared_pa_state_schema_v1_rejects_non_object_decision_tree_children() {
    let pa_state_spec = shared_pa_state_bar_v1();
    let decision_tree_state = &pa_state_spec.output_json_schema["properties"]["decision_tree_state"];

    for field in [
        "trend_context",
        "location_context",
        "signal_quality",
        "confirmation_state",
        "invalidation_conditions",
        "bias_balance",
    ] {
        assert_eq!(
            decision_tree_state["properties"][field]["type"],
            serde_json::json!("object")
        );
    }
}

#[test]
fn shared_bar_analysis_prompt_v2_requires_named_schema_sections() {
    let prompt = shared_bar_analysis_prompt_v2();
    let instructions = prompt.developer_instructions.join("\n");

    assert!(instructions.contains("bar_identity"));
    assert!(instructions.contains("bar_summary"));
    assert!(instructions.contains("bullish_case"));
    assert!(instructions.contains("bearish_case"));
    assert!(instructions.contains("bullish_path or bearish_path"));
    assert!(instructions.contains("Return JSON only"));
}

#[test]
fn shared_daily_context_prompt_v2_requires_single_object_decision_tree() {
    let prompt = shared_daily_context_prompt_v2();
    let instructions = prompt.developer_instructions.join("\n");

    assert!(instructions.contains("context_identity"));
    assert!(instructions.contains("dominant_structure"));
    assert!(instructions.contains("decision_tree_nodes must be a single JSON object"));
    assert!(instructions.contains("path_of_least_resistance must be a JSON object"));
    assert!(instructions.contains("signal_bars must be a JSON object"));
    assert!(instructions.contains("scenario_map"));
    assert!(instructions.contains("Return JSON only"));
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
