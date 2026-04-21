use chrono::Utc;
use pa_core::AppError;
use pa_orchestrator::{
    sha256_json, AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus, TaskEnvelope,
};
use serde::Serialize;
use serde_json::Value;
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
    let context_hash = task_defining_context_hash(
        &input.positions_json,
        &input.subscriptions_json,
        &input.shared_bar_analysis_json,
        &input.shared_daily_context_json,
    )?;

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
    let bar_state = validate_supported_bar_state(bar_state)?;

    let dedupe_key = match bar_state {
        AnalysisBarState::Closed => Some(format!(
            "{task_type}:{user_id}:{instrument_id}:{timeframe}:{identity}:{prompt_key}:{prompt_version}:{context_hash}",
            task_type = USER_POSITION_ADVICE_PROMPT_METADATA.task_type,
            timeframe = timeframe.as_str(),
            identity = user_analysis_identity(bar_state, bar_open_time, bar_close_time, trading_date)?,
            prompt_key = USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key,
            prompt_version = USER_POSITION_ADVICE_PROMPT_METADATA.prompt_version,
        )),
        AnalysisBarState::Open => None,
        AnalysisBarState::None => unreachable!("validated above"),
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
    let context_hash = task_defining_context_hash(
        &input.positions_json,
        &input.subscriptions_json,
        &input.shared_bar_analysis_json,
        &input.shared_daily_context_json,
    )?;
    let ScheduledUserAnalysisInput {
        schedule_id,
        user_id,
        instrument_id,
        timeframe,
        bar_state,
        bar_open_time,
        bar_close_time,
        trading_date,
        ..
    } = input;
    let bar_state = validate_supported_bar_state(bar_state)?;

    let dedupe_key = Some(format!(
        "user_scheduled_analysis:{schedule_id}:{user_id}:{instrument_id}:{timeframe}:{identity}:{prompt_key}:{prompt_version}:{context_hash}",
        timeframe = timeframe.as_str(),
        identity = user_analysis_identity(bar_state, bar_open_time, bar_close_time, trading_date)?,
        prompt_key = USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key,
        prompt_version = USER_POSITION_ADVICE_PROMPT_METADATA.prompt_version,
    ));

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

fn validate_supported_bar_state(bar_state: AnalysisBarState) -> Result<AnalysisBarState, AppError> {
    match bar_state {
        AnalysisBarState::Open | AnalysisBarState::Closed => Ok(bar_state),
        AnalysisBarState::None => Err(AppError::Analysis {
            message: "user position advice tasks require bar_state=open or bar_state=closed"
                .to_string(),
            source: None,
        }),
    }
}

fn task_defining_context_hash(
    positions_json: &Value,
    subscriptions_json: &Value,
    shared_bar_analysis_json: &Value,
    shared_daily_context_json: &Value,
) -> Result<String, AppError> {
    sha256_json(&serde_json::json!({
        "positions": positions_json,
        "subscriptions": subscriptions_json,
        "shared_bar_analysis": shared_bar_analysis_json,
        "shared_daily_context": shared_daily_context_json,
    }))
}

fn user_analysis_identity(
    bar_state: AnalysisBarState,
    bar_open_time: Option<chrono::DateTime<Utc>>,
    bar_close_time: Option<chrono::DateTime<Utc>>,
    trading_date: Option<chrono::NaiveDate>,
) -> Result<String, AppError> {
    match bar_state {
        AnalysisBarState::Open => {
            let bar_open_time = bar_open_time.ok_or_else(|| AppError::Analysis {
                message: "open user analysis tasks require bar_open_time".to_string(),
                source: None,
            })?;

            Ok(format!(
                "open:bar_open_time={bar_open_time}:trading_date={trading_date}",
                bar_open_time = bar_open_time.to_rfc3339(),
                trading_date = trading_date
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            ))
        }
        AnalysisBarState::Closed => {
            if let Some(bar_close_time) = bar_close_time {
                return Ok(format!(
                    "closed:bar_close_time={bar_close_time}:trading_date={trading_date}",
                    bar_close_time = bar_close_time.to_rfc3339(),
                    trading_date = trading_date
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                ));
            }

            if let Some(trading_date) = trading_date {
                return Ok(format!("closed:trading_date={trading_date}"));
            }

            Err(AppError::Analysis {
                message: "closed user analysis tasks require bar_close_time or trading_date"
                    .to_string(),
                source: None,
            })
        }
        AnalysisBarState::None => Err(AppError::Analysis {
            message: "user position advice tasks require bar_state=open or bar_state=closed"
                .to_string(),
            source: None,
        }),
    }
}
