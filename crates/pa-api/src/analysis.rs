use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use pa_analysis::{
    SharedBarAnalysisInput, SharedDailyContextInput, build_shared_bar_analysis_task,
    build_shared_daily_context_task,
};
use pa_core::AppError;
use pa_orchestrator::{
    AnalysisAttempt, AnalysisDeadLetter, AnalysisResult, AnalysisTask, InsertTaskResult,
    OrchestrationRepository,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    router::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/shared/bar", post(create_shared_bar_task))
        .route("/shared/daily", post(create_shared_daily_task))
        .route("/tasks/{task_id}", get(get_task))
        .route("/results/{task_id}", get(get_result))
        .route("/tasks/{task_id}/attempts", get(get_attempts))
        .route("/dead-letters/{task_id}", get(get_dead_letter))
}

pub(crate) fn create_task_response_json(task: &AnalysisTask, dedupe_hit: bool) -> Value {
    json!({
        "task_id": task.id,
        "snapshot_id": task.snapshot_id,
        "task_type": task.task_type,
        "status": task.status.as_str(),
        "dedupe_hit": dedupe_hit,
    })
}

async fn create_shared_bar_task(
    State(state): State<AppState>,
    Json(request): Json<SharedBarAnalysisInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let envelope = build_shared_bar_analysis_task(request)?;
    enqueue_task(&state, envelope.task, envelope.snapshot).await
}

async fn create_shared_daily_task(
    State(state): State<AppState>,
    Json(request): Json<SharedDailyContextInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let envelope = build_shared_daily_context_task(request)?;
    enqueue_task(&state, envelope.task, envelope.snapshot).await
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let task = state
        .orchestration_repository
        .task(task_id)
        .ok_or_else(|| ApiError::not_found(format!("analysis task not found: {task_id}")))?;

    Ok(Json(task_json(&task)))
}

async fn get_result(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let result = state
        .orchestration_repository
        .result_for_task(task_id)
        .ok_or_else(|| ApiError::not_found(format!("analysis result not found: {task_id}")))?;

    Ok(Json(result_json(&result)))
}

async fn get_attempts(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let attempts = state.orchestration_repository.attempts_for_task(task_id);

    Ok(Json(json!({
        "task_id": task_id,
        "attempts": attempts.iter().map(attempt_json).collect::<Vec<_>>(),
    })))
}

async fn get_dead_letter(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let dead_letter = state
        .orchestration_repository
        .dead_letter_for_task(task_id)
        .ok_or_else(|| ApiError::not_found(format!("analysis dead letter not found: {task_id}")))?;

    Ok(Json(dead_letter_json(&dead_letter)))
}

async fn enqueue_task(
    state: &AppState,
    task: AnalysisTask,
    snapshot: pa_orchestrator::AnalysisSnapshot,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let response = match state
        .orchestration_repository
        .insert_task_with_snapshot(task.clone(), snapshot)
        .await?
    {
        InsertTaskResult::Inserted => create_task_response_json(&task, false),
        InsertTaskResult::DuplicateExistingTask(existing_task_id) => {
            let existing_task = state
                .orchestration_repository
                .task(existing_task_id)
                .ok_or_else(|| {
                    ApiError::from(AppError::Storage {
                        message: format!(
                            "dedupe returned missing analysis task: {existing_task_id}"
                        ),
                        source: None,
                    })
                })?;
            create_task_response_json(&existing_task, true)
        }
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}

fn task_json(task: &AnalysisTask) -> Value {
    json!({
        "task_id": task.id,
        "task_type": task.task_type,
        "status": task.status.as_str(),
        "instrument_id": task.instrument_id,
        "user_id": task.user_id,
        "timeframe": task.timeframe.map(|timeframe| timeframe.as_str().to_string()),
        "bar_state": task.bar_state.as_str(),
        "bar_open_time": task.bar_open_time.map(|value| value.to_rfc3339()),
        "bar_close_time": task.bar_close_time.map(|value| value.to_rfc3339()),
        "trading_date": task.trading_date.map(|value| value.to_string()),
        "trigger_type": task.trigger_type,
        "prompt_key": task.prompt_key,
        "prompt_version": task.prompt_version,
        "snapshot_id": task.snapshot_id,
        "dedupe_key": task.dedupe_key,
        "attempt_count": task.attempt_count,
        "max_attempts": task.max_attempts,
        "scheduled_at": task.scheduled_at.to_rfc3339(),
        "started_at": task.started_at.map(|value| value.to_rfc3339()),
        "finished_at": task.finished_at.map(|value| value.to_rfc3339()),
        "last_error_code": task.last_error_code,
        "last_error_message": task.last_error_message,
    })
}

fn result_json(result: &AnalysisResult) -> Value {
    json!({
        "task_id": result.task_id,
        "task_type": result.task_type,
        "instrument_id": result.instrument_id,
        "user_id": result.user_id,
        "timeframe": result.timeframe.map(|timeframe| timeframe.as_str().to_string()),
        "bar_state": result.bar_state.as_str(),
        "bar_open_time": result.bar_open_time.map(|value| value.to_rfc3339()),
        "bar_close_time": result.bar_close_time.map(|value| value.to_rfc3339()),
        "trading_date": result.trading_date.map(|value| value.to_string()),
        "prompt_key": result.prompt_key,
        "prompt_version": result.prompt_version,
        "output_json": result.output_json,
        "created_at": result.created_at.to_rfc3339(),
    })
}

fn attempt_json(attempt: &AnalysisAttempt) -> Value {
    json!({
        "attempt_id": attempt.id,
        "task_id": attempt.task_id,
        "attempt_no": attempt.attempt_no,
        "worker_id": attempt.worker_id,
        "llm_provider": attempt.llm_provider,
        "model": attempt.model,
        "request_payload_json": attempt.request_payload_json,
        "raw_response_json": attempt.raw_response_json,
        "parsed_output_json": attempt.parsed_output_json,
        "status": attempt.status,
        "error_type": attempt.error_type,
        "error_message": attempt.error_message,
        "started_at": attempt.started_at.to_rfc3339(),
        "finished_at": attempt.finished_at.map(|value| value.to_rfc3339()),
    })
}

fn dead_letter_json(dead_letter: &AnalysisDeadLetter) -> Value {
    json!({
        "dead_letter_id": dead_letter.id,
        "task_id": dead_letter.task_id,
        "final_error_type": dead_letter.final_error_type,
        "final_error_message": dead_letter.final_error_message,
        "last_attempt_id": dead_letter.last_attempt_id,
        "archived_snapshot_json": dead_letter.archived_snapshot_json,
        "created_at": dead_letter.created_at.to_rfc3339(),
    })
}
