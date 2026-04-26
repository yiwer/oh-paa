use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use pa_orchestrator::InsertTaskResult;
use pa_user::build_manual_user_analysis_task;
use serde_json::Value;

use crate::{
    analysis::create_task_response_json,
    analysis_runtime::{ManualUserTaskRequest, resolve_manual_user_input},
    error::{ApiError, ApiResult},
    router::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/analysis/manual", post(create_manual_user_analysis_task))
}

async fn create_manual_user_analysis_task(
    State(state): State<AppState>,
    Json(request): Json<ManualUserTaskRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let input = resolve_manual_user_input(&state, request).await?;
    let envelope = build_manual_user_analysis_task(input)?;
    let response = match state
        .orchestration_repository
        .insert_task_with_snapshot(envelope.task.clone(), envelope.snapshot)
        .await?
    {
        InsertTaskResult::Inserted => create_task_response_json(&envelope.task, false),
        InsertTaskResult::DuplicateExistingTask(existing_task_id) => {
            let existing_task = state
                .orchestration_repository
                .task(existing_task_id)
                .await?
                .ok_or_else(|| {
                    ApiError::not_found(format!(
                        "deduped manual analysis task not found: {existing_task_id}"
                    ))
                })?;
            create_task_response_json(&existing_task, true)
        }
    };

    Ok((StatusCode::ACCEPTED, Json(response)))
}
