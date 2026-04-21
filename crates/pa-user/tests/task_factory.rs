use chrono::{NaiveDate, TimeZone, Utc};
use pa_core::Timeframe;
use pa_core::AppError;
use pa_orchestrator::{sha256_json, AnalysisBarState};
use pa_user::{
    build_manual_user_analysis_task, build_scheduled_user_analysis_task, user_position_advice_v1,
    ManualUserAnalysisInput, ScheduledUserAnalysisInput,
};
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

#[test]
fn closed_manual_user_task_dedupe_reflects_task_defining_context_and_open_task_does_not() {
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let bar_open_time = Utc.with_ymd_and_hms(2026, 4, 21, 1, 0, 0).unwrap();
    let bar_close_time = Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap();
    let input = ManualUserAnalysisInput {
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: Some(bar_open_time),
        bar_close_time: Some(bar_close_time),
        trading_date: Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([{
            "side": "long",
            "quantity": Decimal::new(1, 0),
            "average_cost": Decimal::new(100, 0)
        }]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let closed = build_manual_user_analysis_task(input.clone()).unwrap();
    let changed_subscriptions = build_manual_user_analysis_task(ManualUserAnalysisInput {
        subscriptions_json: serde_json::json!([{ "enabled": true }]),
        ..input.clone()
    })
    .unwrap();
    let changed_shared_daily = build_manual_user_analysis_task(ManualUserAnalysisInput {
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}, "risk_notes": {"note": "changed"}}),
        ..input.clone()
    })
    .unwrap();
    let open = build_manual_user_analysis_task(ManualUserAnalysisInput {
        bar_state: AnalysisBarState::Open,
        ..input
    })
    .unwrap();

    assert_eq!(closed.task.trigger_type, "manual");
    assert_eq!(closed.task.bar_state, AnalysisBarState::Closed);
    assert!(closed
        .task
        .dedupe_key
        .as_deref()
        .is_some_and(|key| key.contains(&user_position_advice_v1().prompt_version)));
    assert!(closed
        .task
        .dedupe_key
        .as_deref()
        .is_some_and(|key| key.contains(&bar_close_time.to_rfc3339())));
    assert_ne!(closed.task.dedupe_key, changed_subscriptions.task.dedupe_key);
    assert_ne!(closed.task.dedupe_key, changed_shared_daily.task.dedupe_key);
    assert_eq!(open.task.dedupe_key, None);
}

#[test]
fn manual_user_task_snapshot_serializes_positions_and_shared_outputs() {
    let input = ManualUserAnalysisInput {
        user_id: Uuid::new_v4(),
        instrument_id: Uuid::new_v4(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::Open,
        bar_open_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 1, 45, 0).unwrap()),
        bar_close_time: Some(Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap()),
        trading_date: Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([{
            "side": "short",
            "quantity": Decimal::new(25, 1),
            "average_cost": Decimal::new(43125, 2)
        }]),
        subscriptions_json: serde_json::json!([{"enabled": true}]),
        shared_bar_analysis_json: serde_json::json!({
            "bar_state": "open",
            "bullish_case": {"summary": "test"},
            "bearish_case": {"summary": "test"}
        }),
        shared_daily_context_json: serde_json::json!({
            "decision_tree_nodes": {"trend_context": {}},
            "risk_notes": {}
        }),
    };

    let envelope = build_manual_user_analysis_task(input.clone()).unwrap();
    assert_eq!(envelope.task.task_type, "user_position_advice");
    assert_eq!(envelope.task.prompt_key, "user_position_advice");
    assert_eq!(envelope.task.prompt_version, "v1");
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
fn manual_user_task_rejects_none_bar_state() {
    let input = ManualUserAnalysisInput {
        user_id: Uuid::new_v4(),
        instrument_id: Uuid::new_v4(),
        timeframe: Timeframe::M15,
        bar_state: AnalysisBarState::None,
        bar_open_time: None,
        bar_close_time: None,
        trading_date: Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let error = build_manual_user_analysis_task(input).unwrap_err();
    match error {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("bar_state"));
            assert!(message.contains("open"));
            assert!(message.contains("closed"));
        }
        other => panic!("expected analysis error, got {other:?}"),
    }
}

#[test]
fn scheduled_user_task_uses_supported_bar_state_and_dedupe_reflects_schedule_context() {
    let schedule_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let trading_date = Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap());
    let bar_close_time = Some(Utc.with_ymd_and_hms(2026, 4, 21, 2, 0, 0).unwrap());
    let input = ScheduledUserAnalysisInput {
        schedule_id,
        user_id,
        instrument_id,
        timeframe: Timeframe::H1,
        bar_state: AnalysisBarState::Closed,
        bar_open_time: None,
        bar_close_time,
        trading_date,
        positions_json: serde_json::json!([]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let envelope = build_scheduled_user_analysis_task(input.clone()).unwrap();
    let changed_shared_bar = build_scheduled_user_analysis_task(ScheduledUserAnalysisInput {
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}, "version": 2}),
        ..input
    })
    .unwrap();
    assert_eq!(envelope.task.trigger_type, "schedule");
    assert_eq!(envelope.task.bar_state, AnalysisBarState::Closed);
    assert_eq!(envelope.task.bar_close_time, bar_close_time);
    assert_eq!(envelope.task.trading_date, trading_date);
    assert!(envelope
        .task
        .dedupe_key
        .as_deref()
        .is_some_and(|key| key.contains(&schedule_id.to_string())));
    assert!(envelope
        .task
        .dedupe_key
        .as_deref()
        .is_some_and(|key| key.contains(&user_position_advice_v1().prompt_version)));
    assert_ne!(envelope.task.dedupe_key, changed_shared_bar.task.dedupe_key);
}

#[test]
fn scheduled_user_task_rejects_none_bar_state() {
    let input = ScheduledUserAnalysisInput {
        schedule_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        instrument_id: Uuid::new_v4(),
        timeframe: Timeframe::H1,
        bar_state: AnalysisBarState::None,
        bar_open_time: None,
        bar_close_time: None,
        trading_date: Some(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()),
        positions_json: serde_json::json!([]),
        subscriptions_json: serde_json::json!([]),
        shared_bar_analysis_json: serde_json::json!({"bullish_case": {}, "bearish_case": {}}),
        shared_daily_context_json: serde_json::json!({"decision_tree_nodes": {}}),
    };

    let error = build_scheduled_user_analysis_task(input).unwrap_err();
    match error {
        AppError::Analysis { message, .. } => {
            assert!(message.contains("bar_state"));
            assert!(message.contains("open"));
            assert!(message.contains("closed"));
        }
        other => panic!("expected analysis error, got {other:?}"),
    }
}

#[test]
fn user_prompt_spec_includes_required_pa_contract_fields() {
    let spec = user_position_advice_v1();
    let required = required_fields(&spec.output_json_schema);

    for field in [
        "position_state",
        "market_read_through",
        "bullish_path_for_user",
        "bearish_path_for_user",
        "hold_reduce_exit_conditions",
        "risk_control_levels",
        "invalidations",
        "action_candidates",
    ] {
        assert!(required.contains(&field.to_string()));
    }

    assert_eq!(
        spec.bar_state_support,
        vec![AnalysisBarState::Open, AnalysisBarState::Closed]
    );
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
