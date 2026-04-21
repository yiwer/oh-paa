use chrono::Utc;
use pa_core::AppError;
use uuid::Uuid;

use crate::{
    AnalysisAttempt, AnalysisDeadLetter, AnalysisResult, AnalysisTask, ExecutionAttempt,
    ExecutionOutcome, Executor, LlmClient, OrchestrationRepository, RetryDecision, classify_retry,
};

const DEFAULT_WORKER_ID: &str = "in-memory-worker";

pub async fn run_single_task<R, C>(repository: &R, executor: &Executor<C>) -> Result<bool, AppError>
where
    R: OrchestrationRepository + ?Sized,
    C: LlmClient,
{
    run_single_task_with_worker_id(repository, executor, DEFAULT_WORKER_ID).await
}

pub async fn run_single_task_with_worker_id<R, C>(
    repository: &R,
    executor: &Executor<C>,
    worker_id: &str,
) -> Result<bool, AppError>
where
    R: OrchestrationRepository + ?Sized,
    C: LlmClient,
{
    let Some(task) = repository.claim_next_pending_task().await? else {
        return Ok(false);
    };
    let snapshot = with_claim_recovery(
        repository,
        task.id,
        repository.load_snapshot(task.snapshot_id),
    )
    .await?;
    let execution = executor
        .execute_json(&task.prompt_key, &task.prompt_version, &snapshot.input_json)
        .await;

    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            match classify_retry(
                &error,
                task.attempt_count.saturating_add(1),
                task.max_attempts,
            ) {
                RetryDecision::RetryNow => {
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.mark_task_retry_waiting(task.id, &error.to_string()),
                    )
                    .await?;
                }
                RetryDecision::FailTerminal => {
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.mark_task_failed(task.id, &error.to_string()),
                    )
                    .await?;
                }
                RetryDecision::MoveToDeadLetter => {
                    let dead_letter =
                        AnalysisDeadLetter::from_task_and_error(&task, &snapshot, &error, None);
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.insert_dead_letter(dead_letter),
                    )
                    .await?;
                }
            }

            return Ok(true);
        }
    };

    match execution {
        ExecutionOutcome::Success(attempt) => {
            let output_json = attempt
                .parsed_output_json
                .clone()
                .unwrap_or(serde_json::Value::Null);
            let attempt_row = build_attempt_row(&task, attempt, worker_id, "succeeded");
            let result = AnalysisResult::from_task(&task, output_json);
            with_claim_recovery(
                repository,
                task.id,
                repository.persist_success_outcome(task.id, attempt_row, result),
            )
            .await?;
        }
        ExecutionOutcome::SchemaValidationFailed(attempt) => {
            let attempt_row =
                build_attempt_row(&task, attempt, worker_id, "schema_validation_failed");
            let error_message = attempt_row.error_message.clone().unwrap_or_else(|| {
                "schema validation failed without additional detail".to_string()
            });
            with_claim_recovery(
                repository,
                task.id,
                repository.persist_schema_validation_failure_outcome(
                    task.id,
                    attempt_row,
                    &error_message,
                ),
            )
            .await?;
        }
        ExecutionOutcome::OutboundCallFailed { attempt, error } => {
            let attempt_row = build_attempt_row(&task, attempt, worker_id, "outbound_failed");
            let attempt_id = attempt_row.id;
            let error_message = attempt_row
                .error_message
                .clone()
                .unwrap_or_else(|| error.to_string());

            match classify_retry(
                &error,
                task.attempt_count.saturating_add(1),
                task.max_attempts,
            ) {
                RetryDecision::RetryNow => {
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.persist_outbound_retry_outcome(
                            task.id,
                            attempt_row,
                            &error_message,
                        ),
                    )
                    .await?;
                }
                RetryDecision::FailTerminal => {
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.persist_outbound_terminal_failure_outcome(
                            task.id,
                            attempt_row,
                            &error_message,
                        ),
                    )
                    .await?;
                }
                RetryDecision::MoveToDeadLetter => {
                    let dead_letter = AnalysisDeadLetter::from_task_and_error(
                        &task,
                        &snapshot,
                        &error,
                        Some(attempt_id),
                    );
                    with_claim_recovery(
                        repository,
                        task.id,
                        repository.persist_outbound_dead_letter_outcome(
                            task.id,
                            attempt_row,
                            dead_letter,
                        ),
                    )
                    .await?;
                }
            }
        }
    }

    Ok(true)
}

fn build_attempt_row(
    task: &AnalysisTask,
    attempt: ExecutionAttempt,
    worker_id: &str,
    status: &str,
) -> AnalysisAttempt {
    let now = Utc::now();
    AnalysisAttempt {
        id: Uuid::new_v4(),
        task_id: task.id,
        attempt_no: task.attempt_count.saturating_add(1),
        worker_id: worker_id.to_string(),
        llm_provider: attempt.llm_provider,
        model: attempt.model,
        request_payload_json: attempt.request_payload_json,
        raw_response_json: attempt.raw_response_json,
        parsed_output_json: attempt.parsed_output_json.clone(),
        status: status.to_string(),
        error_type: attempt_error_type(status).map(str::to_string),
        error_message: attempt
            .schema_validation_error
            .or(attempt.outbound_error_message),
        started_at: now,
        finished_at: Some(now),
    }
}

fn attempt_error_type(status: &str) -> Option<&'static str> {
    match status {
        "schema_validation_failed" => Some("validation"),
        "outbound_failed" => Some("provider"),
        _ => None,
    }
}

async fn with_claim_recovery<R, T>(
    repository: &R,
    task_id: Uuid,
    operation: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppError>
where
    R: OrchestrationRepository + ?Sized,
{
    match operation.await {
        Ok(value) => Ok(value),
        Err(err) => {
            let _ = repository
                .release_claimed_task(task_id, &err.to_string())
                .await;
            Err(err)
        }
    }
}
