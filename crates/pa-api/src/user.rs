use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use pa_orchestrator::{InsertTaskResult, OrchestrationRepository};
use pa_user::{ManualUserAnalysisInput, build_manual_user_analysis_task};
use serde_json::Value;

use crate::{
    analysis::create_task_response_json,
    error::{ApiError, ApiResult},
    router::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/analysis/manual", post(create_manual_user_analysis_task))
}

async fn create_manual_user_analysis_task(
    State(state): State<AppState>,
    Json(request): Json<ManualUserAnalysisInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let envelope = build_manual_user_analysis_task(request)?;
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
