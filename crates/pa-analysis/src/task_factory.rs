use chrono::Utc;
use pa_core::AppError;
use pa_orchestrator::{
    AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus, TaskEnvelope,
    build_shared_bar_dedupe_key, sha256_json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    SharedBarAnalysisInput, SharedDailyContextInput, SharedPaStateBarInput,
    prompt_specs::{
        SHARED_BAR_ANALYSIS_PROMPT_METADATA, SHARED_DAILY_CONTEXT_PROMPT_METADATA,
        SHARED_PA_STATE_BAR_PROMPT_METADATA,
    },
};

const DEFAULT_MAX_ATTEMPTS: u32 = 3;

pub fn build_shared_bar_analysis_task(
    input: SharedBarAnalysisInput,
) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "shared bar analysis input")?;
    let input_hash = sha256_json(&input_json)?;
    let SharedBarAnalysisInput {
        instrument_id,
        timeframe,
        bar_open_time,
        bar_close_time,
        bar_state,
        ..
    } = input;
    let dedupe_key = build_shared_bar_dedupe_key(
        instrument_id,
        timeframe,
        bar_close_time,
        SHARED_BAR_ANALYSIS_PROMPT_METADATA.prompt_key,
        SHARED_BAR_ANALYSIS_PROMPT_METADATA.prompt_version,
        bar_state,
    );

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: SHARED_BAR_ANALYSIS_PROMPT_METADATA.task_type.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: None,
            timeframe: Some(timeframe),
            bar_state,
            bar_open_time: Some(bar_open_time),
            bar_close_time: Some(bar_close_time),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: SHARED_BAR_ANALYSIS_PROMPT_METADATA.prompt_key.to_string(),
            prompt_version: SHARED_BAR_ANALYSIS_PROMPT_METADATA
                .prompt_version
                .to_string(),
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
            schema_version: SHARED_BAR_ANALYSIS_PROMPT_METADATA
                .input_schema_version
                .to_string(),
            created_at: scheduled_at,
        },
    })
}

pub fn build_shared_daily_context_task(
    input: SharedDailyContextInput,
) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "shared daily context input")?;
    let input_hash = sha256_json(&input_json)?;
    let SharedDailyContextInput {
        instrument_id,
        trading_date,
        ..
    } = input;
    let dedupe_key = Some(format!(
        "{task_type}:{instrument_id}:{trading_date}:{prompt_key}:{prompt_version}",
        task_type = SHARED_DAILY_CONTEXT_PROMPT_METADATA.task_type,
        instrument_id = instrument_id,
        trading_date = trading_date,
        prompt_key = SHARED_DAILY_CONTEXT_PROMPT_METADATA.prompt_key,
        prompt_version = SHARED_DAILY_CONTEXT_PROMPT_METADATA.prompt_version,
    ));

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: SHARED_DAILY_CONTEXT_PROMPT_METADATA.task_type.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: None,
            timeframe: None,
            bar_state: AnalysisBarState::None,
            bar_open_time: None,
            bar_close_time: None,
            trading_date: Some(trading_date),
            trigger_type: "schedule".to_string(),
            prompt_key: SHARED_DAILY_CONTEXT_PROMPT_METADATA.prompt_key.to_string(),
            prompt_version: SHARED_DAILY_CONTEXT_PROMPT_METADATA
                .prompt_version
                .to_string(),
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
            schema_version: SHARED_DAILY_CONTEXT_PROMPT_METADATA
                .input_schema_version
                .to_string(),
            created_at: scheduled_at,
        },
    })
}

pub fn build_shared_pa_state_bar_task(
    input: SharedPaStateBarInput,
) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "shared pa state input")?;
    let input_hash = sha256_json(&input_json)?;
    let SharedPaStateBarInput {
        instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
        ..
    } = input;
    let dedupe_key = match bar_state {
        AnalysisBarState::Closed => Some(format!(
            "{}:{}:{}:{}:{}:{}",
            SHARED_PA_STATE_BAR_PROMPT_METADATA.task_type,
            instrument_id,
            timeframe.as_str(),
            bar_close_time.to_rfc3339(),
            SHARED_PA_STATE_BAR_PROMPT_METADATA.prompt_key,
            SHARED_PA_STATE_BAR_PROMPT_METADATA.prompt_version
        )),
        AnalysisBarState::Open => None,
        AnalysisBarState::None => unreachable!("pa state bar task requires open or closed"),
    };

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: SHARED_PA_STATE_BAR_PROMPT_METADATA.task_type.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: None,
            timeframe: Some(timeframe),
            bar_state,
            bar_open_time: Some(bar_open_time),
            bar_close_time: Some(bar_close_time),
            trading_date: None,
            trigger_type: "event".to_string(),
            prompt_key: SHARED_PA_STATE_BAR_PROMPT_METADATA.prompt_key.to_string(),
            prompt_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
                .prompt_version
                .to_string(),
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
            schema_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
                .input_schema_version
                .to_string(),
            created_at: scheduled_at,
        },
    })
}

fn serialize_snapshot_input<T>(input: &T, label: &str) -> Result<serde_json::Value, AppError>
where
    T: Serialize,
{
    serde_json::to_value(input).map_err(|err| AppError::Analysis {
        message: format!("failed to serialize {label}: {err}"),
        source: None,
    })
}
