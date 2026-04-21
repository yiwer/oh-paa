use chrono::Utc;
use pa_core::AppError;
use pa_orchestrator::{
    sha256_json, AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus, TaskEnvelope,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    prompt_specs::USER_POSITION_ADVICE_PROMPT_METADATA, ManualUserAnalysisInput,
    ScheduledUserAnalysisInput,
};

const DEFAULT_MAX_ATTEMPTS: u32 = 3;

pub fn build_manual_user_analysis_task(input: ManualUserAnalysisInput) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "manual user analysis input")?;
    let input_hash = sha256_json(&input_json)?;
    let position_snapshot_hash = sha256_json(&input.positions_json)?;

    let ManualUserAnalysisInput {
        user_id,
        instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
        trading_date,
        ..
    } = input;

    let dedupe_key = match bar_state {
        AnalysisBarState::Closed => {
            let closed_at = bar_close_time.ok_or_else(|| AppError::Analysis {
                message: "bar_close_time is required for closed manual user analysis tasks"
                    .to_string(),
                source: None,
            })?;

            Some(format!(
                "{task_type}:{user_id}:{instrument_id}:{timeframe}:{bar_close_time}:{prompt_version}:{position_snapshot_hash}:closed",
                task_type = USER_POSITION_ADVICE_PROMPT_METADATA.task_type,
                timeframe = timeframe.as_str(),
                bar_close_time = closed_at.to_rfc3339(),
                prompt_version = USER_POSITION_ADVICE_PROMPT_METADATA.prompt_version,
            ))
        }
        AnalysisBarState::Open => None,
        AnalysisBarState::None => None,
    };

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: USER_POSITION_ADVICE_PROMPT_METADATA.task_type.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: Some(user_id),
            timeframe: Some(timeframe),
            bar_state,
            bar_open_time,
            bar_close_time,
            trading_date,
            trigger_type: "manual".to_string(),
            prompt_key: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key.to_string(),
            prompt_version: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_version.to_string(),
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
            schema_version: USER_POSITION_ADVICE_PROMPT_METADATA
                .input_schema_version
                .to_string(),
            created_at: scheduled_at,
        },
    })
}

pub fn build_scheduled_user_analysis_task(
    input: ScheduledUserAnalysisInput,
) -> Result<TaskEnvelope, AppError> {
    let task_id = Uuid::new_v4();
    let snapshot_id = Uuid::new_v4();
    let scheduled_at = Utc::now();
    let input_json = serialize_snapshot_input(&input, "scheduled user analysis input")?;
    let input_hash = sha256_json(&input_json)?;
    let position_snapshot_hash = sha256_json(&input.positions_json)?;
    let ScheduledUserAnalysisInput {
        schedule_id,
        user_id,
        instrument_id,
        timeframe,
        trading_date,
        ..
    } = input;

    let dedupe_key = Some(format!(
        "user_scheduled_analysis:{schedule_id}:{user_id}:{instrument_id}:{timeframe}:{trading_date}:{position_snapshot_hash}",
        timeframe = timeframe.as_str(),
    ));

    Ok(TaskEnvelope {
        task: AnalysisTask {
            id: task_id,
            task_type: USER_POSITION_ADVICE_PROMPT_METADATA.task_type.to_string(),
            status: AnalysisTaskStatus::Pending,
            instrument_id,
            user_id: Some(user_id),
            timeframe: Some(timeframe),
            bar_state: AnalysisBarState::None,
            bar_open_time: None,
            bar_close_time: None,
            trading_date: Some(trading_date),
            trigger_type: "schedule".to_string(),
            prompt_key: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key.to_string(),
            prompt_version: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_version.to_string(),
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
            schema_version: USER_POSITION_ADVICE_PROMPT_METADATA
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
