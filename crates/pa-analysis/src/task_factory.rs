use chrono::Utc;
use pa_core::AppError;
use pa_orchestrator::{
    build_shared_bar_dedupe_key, sha256_json, AnalysisBarState, AnalysisSnapshot, AnalysisTask,
    AnalysisTaskStatus, TaskEnvelope,
};
use uuid::Uuid;

use crate::{SharedBarAnalysisInput, SharedDailyContextInput};

const SHARED_BAR_PROMPT_KEY: &str = "shared_bar_analysis";
const SHARED_DAILY_PROMPT_KEY: &str = "shared_daily_context";
const PROMPT_VERSION: &str = "v1";
const DEFAULT_MAX_ATTEMPTS: u32 = 3;

pub fn build_shared_bar_analysis_task(input: SharedBarAnalysisInput) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serde_json::json!({
        "instrument_id": input.instrument_id,
        "timeframe": input.timeframe.as_str(),
        "bar_open_time": input.bar_open_time.to_rfc3339(),
        "bar_close_time": input.bar_close_time.to_rfc3339(),
        "bar_state": input.bar_state.as_str(),
        "canonical_bar_json": input.canonical_bar_json,
        "structure_context_json": input.structure_context_json
    });
    let input_hash = sha256_json(&input_json)?;
    let dedupe_key = build_shared_bar_dedupe_key(
        input.instrument_id,
        input.timeframe,
        input.bar_close_time,
        SHARED_BAR_PROMPT_KEY,
        PROMPT_VERSION,
        input.bar_state,
    );

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: SHARED_BAR_PROMPT_KEY.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id: input.instrument_id,
            user_id: None,
            timeframe: Some(input.timeframe),
            bar_state: input.bar_state,
            bar_open_time: Some(input.bar_open_time),
            bar_close_time: Some(input.bar_close_time),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: SHARED_BAR_PROMPT_KEY.to_string(),
            prompt_version: PROMPT_VERSION.to_string(),
            snapshot_id,
            dedupe_key,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            scheduled_at,
            started_at: None,
            finished_at: None,
            last_error_code: None,
            last_error_message: None,
        },
        snapshot: AnalysisSnapshot {
            id: snapshot_id,
            task_id,
            input_json,
            input_hash,
            schema_version: PROMPT_VERSION.to_string(),
            created_at: scheduled_at,
        },
    })
}

pub fn build_shared_daily_context_task(input: SharedDailyContextInput) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serde_json::json!({
        "instrument_id": input.instrument_id,
        "trading_date": input.trading_date.to_string(),
        "m15_structure_json": input.m15_structure_json,
        "h1_structure_json": input.h1_structure_json,
        "d1_structure_json": input.d1_structure_json,
        "recent_shared_bar_analyses_json": input.recent_shared_bar_analyses_json,
        "key_levels_json": input.key_levels_json,
        "signal_bar_candidates_json": input.signal_bar_candidates_json,
        "market_background_json": input.market_background_json
    });
    let input_hash = sha256_json(&input_json)?;
    let dedupe_key = Some(format!(
        "{task_type}:{instrument_id}:{trading_date}:{prompt_key}:{prompt_version}",
        task_type = SHARED_DAILY_PROMPT_KEY,
        instrument_id = input.instrument_id,
        trading_date = input.trading_date,
        prompt_key = SHARED_DAILY_PROMPT_KEY,
        prompt_version = PROMPT_VERSION,
    ));

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: SHARED_DAILY_PROMPT_KEY.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id: input.instrument_id,
            user_id: None,
            timeframe: None,
            bar_state: AnalysisBarState::None,
            bar_open_time: None,
            bar_close_time: None,
            trading_date: Some(input.trading_date),
            trigger_type: "schedule".to_string(),
            prompt_key: SHARED_DAILY_PROMPT_KEY.to_string(),
            prompt_version: PROMPT_VERSION.to_string(),
            snapshot_id,
            dedupe_key,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            scheduled_at,
            started_at: None,
            finished_at: None,
            last_error_code: None,
            last_error_message: None,
        },
        snapshot: AnalysisSnapshot {
            id: snapshot_id,
            task_id,
            input_json,
            input_hash,
            schema_version: PROMPT_VERSION.to_string(),
            created_at: scheduled_at,
        },
    })
}
