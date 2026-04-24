use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::Utc;
use pa_core::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    AnalysisAttempt, AnalysisBarState, AnalysisDeadLetter, AnalysisResult, AnalysisSnapshot,
    AnalysisTask, AnalysisTaskStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertTaskResult {
    Inserted,
    DuplicateExistingTask(Uuid),
}

#[async_trait]
pub trait OrchestrationRepository: Send + Sync {
    async fn insert_task_with_snapshot(
        &self,
        task: AnalysisTask,
        snapshot: AnalysisSnapshot,
    ) -> Result<InsertTaskResult, AppError>;

    async fn task(&self, task_id: Uuid) -> Result<Option<AnalysisTask>, AppError>;

    async fn result_for_task(&self, task_id: Uuid) -> Result<Option<AnalysisResult>, AppError>;

    async fn attempts_for_task(&self, task_id: Uuid) -> Result<Vec<AnalysisAttempt>, AppError>;

    async fn dead_letter_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<AnalysisDeadLetter>, AppError>;

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError>;

    async fn claim_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError>;

    async fn release_claimed_task(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError>;

    async fn persist_success_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        result: AnalysisResult,
    ) -> Result<(), AppError>;

    async fn persist_schema_validation_failure_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError>;

    async fn persist_outbound_retry_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError>;

    async fn persist_outbound_terminal_failure_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError>;

    async fn persist_outbound_dead_letter_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        dead_letter: AnalysisDeadLetter,
    ) -> Result<(), AppError>;

    async fn mark_task_running(&self, task_id: Uuid) -> Result<(), AppError>;

    async fn append_attempt(&self, attempt: AnalysisAttempt) -> Result<(), AppError>;

    async fn mark_task_retry_waiting(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;

    async fn mark_task_failed(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;

    async fn insert_result_and_complete(&self, result: AnalysisResult) -> Result<(), AppError>;

    async fn insert_dead_letter(&self, dead_letter: AnalysisDeadLetter) -> Result<(), AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryOrchestrationRepository {
    state: Mutex<InMemoryState>,
}

#[derive(Debug, Default)]
struct InMemoryState {
    tasks: HashMap<Uuid, AnalysisTask>,
    snapshots: HashMap<Uuid, AnalysisSnapshot>,
    attempts: Vec<AnalysisAttempt>,
    results: Vec<AnalysisResult>,
    dead_letters: Vec<AnalysisDeadLetter>,
    fail_next_outcome_persist: bool,
    pending_order: VecDeque<Uuid>,
    keyed_dedupe: HashMap<String, Uuid>,
}

impl InMemoryOrchestrationRepository {
    fn lock_state(&self) -> MutexGuard<'_, InMemoryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn only_task(&self) -> AnalysisTask {
        let state = self.lock_state();
        assert_eq!(state.tasks.len(), 1, "expected exactly one task");
        state
            .tasks
            .values()
            .next()
            .cloned()
            .expect("one task should exist")
    }

    pub fn attempts(&self) -> Vec<AnalysisAttempt> {
        self.lock_state().attempts.clone()
    }

    pub fn results(&self) -> Vec<AnalysisResult> {
        self.lock_state().results.clone()
    }

    pub fn dead_letters(&self) -> Vec<AnalysisDeadLetter> {
        self.lock_state().dead_letters.clone()
    }

    pub fn task(&self, task_id: Uuid) -> Option<AnalysisTask> {
        self.lock_state().tasks.get(&task_id).cloned()
    }

    pub fn result_for_task(&self, task_id: Uuid) -> Option<AnalysisResult> {
        self.lock_state()
            .results
            .iter()
            .find(|result| result.task_id == task_id)
            .cloned()
    }

    pub fn attempts_for_task(&self, task_id: Uuid) -> Vec<AnalysisAttempt> {
        self.lock_state()
            .attempts
            .iter()
            .filter(|attempt| attempt.task_id == task_id)
            .cloned()
            .collect()
    }

    pub fn dead_letter_for_task(&self, task_id: Uuid) -> Option<AnalysisDeadLetter> {
        self.lock_state()
            .dead_letters
            .iter()
            .find(|dead_letter| dead_letter.task_id == task_id)
            .cloned()
    }

    pub fn remove_snapshot(&self, snapshot_id: Uuid) {
        self.lock_state().snapshots.remove(&snapshot_id);
    }

    pub fn fail_next_outcome_persist(&self) {
        self.lock_state().fail_next_outcome_persist = true;
    }

    fn maybe_fail_outcome_persist(state: &mut InMemoryState) -> Result<(), AppError> {
        if state.fail_next_outcome_persist {
            state.fail_next_outcome_persist = false;
            return Err(AppError::Storage {
                message: "in-memory injected outcome persist failure".to_string(),
                source: None,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl OrchestrationRepository for InMemoryOrchestrationRepository {
    async fn insert_task_with_snapshot(
        &self,
        task: AnalysisTask,
        snapshot: AnalysisSnapshot,
    ) -> Result<InsertTaskResult, AppError> {
        if task.snapshot_id != snapshot.id || task.id != snapshot.task_id {
            return Err(AppError::Validation {
                message: "task/snapshot ownership mismatch".to_string(),
                source: None,
            });
        }

        let mut state = self.lock_state();

        if state.tasks.contains_key(&task.id) || state.snapshots.contains_key(&snapshot.id) {
            return Err(AppError::Storage {
                message: "task or snapshot already exists".to_string(),
                source: None,
            });
        }

        if let Some(dedupe_key) = task.dedupe_key.as_ref()
            && let Some(existing_task_id) = state.keyed_dedupe.get(dedupe_key)
        {
            return Ok(InsertTaskResult::DuplicateExistingTask(*existing_task_id));
        }

        if let Some(dedupe_key) = task.dedupe_key.as_ref() {
            state.keyed_dedupe.insert(dedupe_key.clone(), task.id);
        }

        if matches!(task.status, AnalysisTaskStatus::Pending) {
            state.pending_order.push_back(task.id);
        }

        state.snapshots.insert(snapshot.id, snapshot);
        state.tasks.insert(task.id, task);

        Ok(InsertTaskResult::Inserted)
    }

    async fn task(&self, task_id: Uuid) -> Result<Option<AnalysisTask>, AppError> {
        Ok(self.task(task_id))
    }

    async fn result_for_task(&self, task_id: Uuid) -> Result<Option<AnalysisResult>, AppError> {
        Ok(self.result_for_task(task_id))
    }

    async fn attempts_for_task(&self, task_id: Uuid) -> Result<Vec<AnalysisAttempt>, AppError> {
        Ok(self.attempts_for_task(task_id))
    }

    async fn dead_letter_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<AnalysisDeadLetter>, AppError> {
        Ok(self.dead_letter_for_task(task_id))
    }

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
        let state = self.lock_state();
        for task_id in &state.pending_order {
            if let Some(task) = state.tasks.get(task_id)
                && matches!(task.status, AnalysisTaskStatus::Pending)
            {
                return Ok(Some(task.clone()));
            }
        }

        Ok(None)
    }

    async fn claim_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
        let mut state = self.lock_state();
        for task_id in state.pending_order.clone() {
            if let Some(task) = state.tasks.get_mut(&task_id)
                && matches!(
                    task.status,
                    AnalysisTaskStatus::Pending | AnalysisTaskStatus::RetryWaiting
                )
            {
                task.status = AnalysisTaskStatus::Running;
                if task.started_at.is_none() {
                    task.started_at = Some(Utc::now());
                }
                return Ok(Some(task.clone()));
            }
        }

        Ok(None)
    }

    async fn release_claimed_task(&self, task_id: Uuid, message: &str) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if matches!(task.status, AnalysisTaskStatus::Running) {
            task.status = AnalysisTaskStatus::Pending;
        }
        task.last_error_code = Some("claim_released".to_string());
        task.last_error_message = Some(message.to_string());

        Ok(())
    }

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError> {
        self.lock_state()
            .snapshots
            .get(&snapshot_id)
            .cloned()
            .ok_or_else(|| AppError::Storage {
                message: format!("snapshot not found: {snapshot_id}"),
                source: None,
            })
    }

    async fn persist_success_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        result: AnalysisResult,
    ) -> Result<(), AppError> {
        let mut state = self.lock_state();
        Self::maybe_fail_outcome_persist(&mut state)?;
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if attempt.task_id != task_id || result.task_id != task_id {
            return Err(AppError::Storage {
                message: format!("outcome/task mismatch for task: {task_id}"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::Succeeded;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_code = None;
        task.last_error_message = None;
        task.finished_at = Some(Utc::now());
        state.attempts.push(attempt);
        state.results.push(result);
        Ok(())
    }

    async fn persist_schema_validation_failure_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError> {
        let mut state = self.lock_state();
        Self::maybe_fail_outcome_persist(&mut state)?;
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if attempt.task_id != task_id {
            return Err(AppError::Storage {
                message: format!("attempt/task mismatch for task: {task_id}"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::Failed;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_message = Some(message.to_string());
        task.last_error_code = Some("terminal_error".to_string());
        task.finished_at = Some(Utc::now());
        state.attempts.push(attempt);
        Ok(())
    }

    async fn persist_outbound_retry_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError> {
        let mut state = self.lock_state();
        Self::maybe_fail_outcome_persist(&mut state)?;
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if attempt.task_id != task_id {
            return Err(AppError::Storage {
                message: format!("attempt/task mismatch for task: {task_id}"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::RetryWaiting;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_message = Some(message.to_string());
        task.last_error_code = Some("retryable_error".to_string());
        state.attempts.push(attempt);
        Ok(())
    }

    async fn persist_outbound_terminal_failure_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        message: &str,
    ) -> Result<(), AppError> {
        let mut state = self.lock_state();
        Self::maybe_fail_outcome_persist(&mut state)?;
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if attempt.task_id != task_id {
            return Err(AppError::Storage {
                message: format!("attempt/task mismatch for task: {task_id}"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::Failed;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_message = Some(message.to_string());
        task.last_error_code = Some("terminal_error".to_string());
        task.finished_at = Some(Utc::now());
        state.attempts.push(attempt);
        Ok(())
    }

    async fn persist_outbound_dead_letter_outcome(
        &self,
        task_id: Uuid,
        attempt: AnalysisAttempt,
        dead_letter: AnalysisDeadLetter,
    ) -> Result<(), AppError> {
        let mut state = self.lock_state();
        Self::maybe_fail_outcome_persist(&mut state)?;
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if attempt.task_id != task_id || dead_letter.task_id != task_id {
            return Err(AppError::Storage {
                message: format!("outcome/task mismatch for task: {task_id}"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::DeadLetter;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_code = Some(dead_letter.final_error_type.clone());
        task.last_error_message = Some(dead_letter.final_error_message.clone());
        task.finished_at = Some(Utc::now());
        state.attempts.push(attempt);
        state.dead_letters.push(dead_letter);
        Ok(())
    }

    async fn mark_task_running(&self, task_id: Uuid) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        if !matches!(task.status, AnalysisTaskStatus::Pending) {
            return Err(AppError::Storage {
                message: format!("task {task_id} is not pending"),
                source: None,
            });
        }

        task.status = AnalysisTaskStatus::Running;
        if task.started_at.is_none() {
            task.started_at = Some(Utc::now());
        }

        Ok(())
    }

    async fn append_attempt(&self, attempt: AnalysisAttempt) -> Result<(), AppError> {
        let mut state = self.lock_state();
        if !state.tasks.contains_key(&attempt.task_id) {
            return Err(AppError::Storage {
                message: format!("task not found for attempt: {}", attempt.task_id),
                source: None,
            });
        }

        state.attempts.push(attempt);
        Ok(())
    }

    async fn mark_task_retry_waiting(&self, task_id: Uuid, message: &str) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        task.status = AnalysisTaskStatus::RetryWaiting;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_message = Some(message.to_string());
        task.last_error_code = Some("retryable_error".to_string());

        Ok(())
    }

    async fn mark_task_failed(&self, task_id: Uuid, message: &str) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found: {task_id}"),
                source: None,
            })?;

        task.status = AnalysisTaskStatus::Failed;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_message = Some(message.to_string());
        task.last_error_code = Some("terminal_error".to_string());
        task.finished_at = Some(Utc::now());

        Ok(())
    }

    async fn insert_result_and_complete(&self, result: AnalysisResult) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&result.task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found for result: {}", result.task_id),
                source: None,
            })?;

        task.status = AnalysisTaskStatus::Succeeded;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_code = None;
        task.last_error_message = None;
        task.finished_at = Some(Utc::now());
        state.results.push(result);

        Ok(())
    }

    async fn insert_dead_letter(&self, dead_letter: AnalysisDeadLetter) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state
            .tasks
            .get_mut(&dead_letter.task_id)
            .ok_or_else(|| AppError::Storage {
                message: format!("task not found for dead letter: {}", dead_letter.task_id),
                source: None,
            })?;

        task.status = AnalysisTaskStatus::DeadLetter;
        task.attempt_count = task.attempt_count.saturating_add(1);
        task.last_error_code = Some(dead_letter.final_error_type.clone());
        task.last_error_message = Some(dead_letter.final_error_message.clone());
        task.finished_at = Some(Utc::now());
        state.dead_letters.push(dead_letter);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PgOrchestrationRepository {
    pool: PgPool,
}

impl PgOrchestrationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn unsupported(method: &str) -> AppError {
        AppError::Storage {
            message: format!(
                "{method} is not implemented for PgOrchestrationRepository in Task 1"
            ),
            source: None,
        }
    }
}

#[async_trait]
impl OrchestrationRepository for PgOrchestrationRepository {
    async fn insert_task_with_snapshot(
        &self,
        task: AnalysisTask,
        snapshot: AnalysisSnapshot,
    ) -> Result<InsertTaskResult, AppError> {
        if task.snapshot_id != snapshot.id || task.id != snapshot.task_id {
            return Err(AppError::Validation {
                message: "task/snapshot ownership mismatch".to_string(),
                source: None,
            });
        }

        let mut tx = self.pool.begin().await.map_err(storage_error(
            "failed to begin task and snapshot insert transaction",
        ))?;

        if let Some(dedupe_key) = task.dedupe_key.as_ref() {
            if let Some(existing_task_id) = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM analysis_tasks WHERE dedupe_key = $1 LIMIT 1",
            )
            .bind(dedupe_key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(storage_error("failed to query existing dedupe task"))?
            {
                tx.rollback()
                    .await
                    .map_err(storage_error("failed to rollback duplicate insert transaction"))?;
                return Ok(InsertTaskResult::DuplicateExistingTask(existing_task_id));
            }
        }

        sqlx::query(
            r#"
            INSERT INTO analysis_tasks (
                id,
                task_type,
                status,
                instrument_id,
                user_id,
                timeframe,
                bar_state,
                bar_open_time,
                bar_close_time,
                trading_date,
                trigger_type,
                prompt_key,
                prompt_version,
                snapshot_id,
                dedupe_key,
                attempt_count,
                max_attempts,
                scheduled_at,
                started_at,
                finished_at,
                last_error_code,
                last_error_message
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            )
            "#,
        )
        .bind(task.id)
        .bind(&task.task_type)
        .bind(task.status.as_str())
        .bind(task.instrument_id)
        .bind(task.user_id)
        .bind(task.timeframe.map(|value| value.as_str().to_string()))
        .bind(task.bar_state.as_str())
        .bind(task.bar_open_time)
        .bind(task.bar_close_time)
        .bind(task.trading_date)
        .bind(&task.trigger_type)
        .bind(&task.prompt_key)
        .bind(&task.prompt_version)
        .bind(task.snapshot_id)
        .bind(task.dedupe_key.as_deref())
        .bind(i32::try_from(task.attempt_count).map_err(|source| AppError::Validation {
            message: "attempt_count exceeds PostgreSQL integer range".to_string(),
            source: Some(Box::new(source)),
        })?)
        .bind(i32::try_from(task.max_attempts).map_err(|source| AppError::Validation {
            message: "max_attempts exceeds PostgreSQL integer range".to_string(),
            source: Some(Box::new(source)),
        })?)
        .bind(task.scheduled_at)
        .bind(task.started_at)
        .bind(task.finished_at)
        .bind(task.last_error_code.as_deref())
        .bind(task.last_error_message.as_deref())
        .execute(tx.as_mut())
        .await
        .map_err(storage_error("failed to insert analysis task"))?;

        sqlx::query(
            r#"
            INSERT INTO analysis_snapshots (
                id,
                task_id,
                input_json,
                input_hash,
                schema_version,
                created_at
            ) VALUES ($1, $2, CAST($3 AS jsonb), $4, $5, $6)
            "#,
        )
        .bind(snapshot.id)
        .bind(snapshot.task_id)
        .bind(snapshot.input_json.to_string())
        .bind(&snapshot.input_hash)
        .bind(&snapshot.schema_version)
        .bind(snapshot.created_at)
        .execute(tx.as_mut())
        .await
        .map_err(storage_error("failed to insert analysis snapshot"))?;

        tx.commit()
            .await
            .map_err(storage_error("failed to commit task and snapshot insert transaction"))?;

        Ok(InsertTaskResult::Inserted)
    }

    async fn task(&self, task_id: Uuid) -> Result<Option<AnalysisTask>, AppError> {
        sqlx::query(
            r#"
            SELECT
                id,
                task_type,
                status,
                instrument_id,
                user_id,
                timeframe,
                bar_state,
                bar_open_time,
                bar_close_time,
                trading_date,
                trigger_type,
                prompt_key,
                prompt_version,
                snapshot_id,
                dedupe_key,
                attempt_count,
                max_attempts,
                scheduled_at,
                started_at,
                finished_at,
                last_error_code,
                last_error_message
            FROM analysis_tasks
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error("failed to query analysis task"))?
        .map(map_task_row)
        .transpose()
    }

    async fn result_for_task(&self, task_id: Uuid) -> Result<Option<AnalysisResult>, AppError> {
        sqlx::query(
            r#"
            SELECT
                id,
                task_id,
                task_type,
                instrument_id,
                user_id,
                timeframe,
                bar_state,
                bar_open_time,
                bar_close_time,
                trading_date,
                prompt_key,
                prompt_version,
                output_json::text AS output_json_text,
                created_at
            FROM analysis_results
            WHERE task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error("failed to query analysis result"))?
        .map(map_result_row)
        .transpose()
    }

    async fn attempts_for_task(&self, task_id: Uuid) -> Result<Vec<AnalysisAttempt>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                task_id,
                attempt_no,
                worker_id,
                llm_provider,
                model,
                request_payload_json::text AS request_payload_json_text,
                raw_response_json::text AS raw_response_json_text,
                parsed_output_json::text AS parsed_output_json_text,
                status,
                error_type,
                error_message,
                started_at,
                finished_at
            FROM analysis_attempts
            WHERE task_id = $1
            ORDER BY attempt_no ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error("failed to query analysis attempts"))?;

        rows.into_iter().map(map_attempt_row).collect()
    }

    async fn dead_letter_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<AnalysisDeadLetter>, AppError> {
        sqlx::query(
            r#"
            SELECT
                id,
                task_id,
                final_error_type,
                final_error_message,
                last_attempt_id,
                archived_snapshot_json::text AS archived_snapshot_json_text,
                created_at
            FROM analysis_dead_letters
            WHERE task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error("failed to query analysis dead letter"))?
        .map(map_dead_letter_row)
        .transpose()
    }

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
        Err(Self::unsupported("fetch_next_pending_task"))
    }

    async fn claim_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
        Err(Self::unsupported("claim_next_pending_task"))
    }

    async fn release_claimed_task(&self, _task_id: Uuid, _message: &str) -> Result<(), AppError> {
        Err(Self::unsupported("release_claimed_task"))
    }

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError> {
        sqlx::query(
            r#"
            SELECT
                id,
                task_id,
                input_json::text AS input_json_text,
                input_hash,
                schema_version,
                created_at
            FROM analysis_snapshots
            WHERE id = $1
            "#,
        )
        .bind(snapshot_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error("failed to query analysis snapshot"))?
        .map(map_snapshot_row)
        .transpose()?
        .ok_or_else(|| AppError::Storage {
            message: format!("snapshot not found: {snapshot_id}"),
            source: None,
        })
    }

    async fn persist_success_outcome(
        &self,
        _task_id: Uuid,
        _attempt: AnalysisAttempt,
        _result: AnalysisResult,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("persist_success_outcome"))
    }

    async fn persist_schema_validation_failure_outcome(
        &self,
        _task_id: Uuid,
        _attempt: AnalysisAttempt,
        _message: &str,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("persist_schema_validation_failure_outcome"))
    }

    async fn persist_outbound_retry_outcome(
        &self,
        _task_id: Uuid,
        _attempt: AnalysisAttempt,
        _message: &str,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("persist_outbound_retry_outcome"))
    }

    async fn persist_outbound_terminal_failure_outcome(
        &self,
        _task_id: Uuid,
        _attempt: AnalysisAttempt,
        _message: &str,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("persist_outbound_terminal_failure_outcome"))
    }

    async fn persist_outbound_dead_letter_outcome(
        &self,
        _task_id: Uuid,
        _attempt: AnalysisAttempt,
        _dead_letter: AnalysisDeadLetter,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("persist_outbound_dead_letter_outcome"))
    }

    async fn mark_task_running(&self, _task_id: Uuid) -> Result<(), AppError> {
        Err(Self::unsupported("mark_task_running"))
    }

    async fn append_attempt(&self, _attempt: AnalysisAttempt) -> Result<(), AppError> {
        Err(Self::unsupported("append_attempt"))
    }

    async fn mark_task_retry_waiting(
        &self,
        _task_id: Uuid,
        _message: &str,
    ) -> Result<(), AppError> {
        Err(Self::unsupported("mark_task_retry_waiting"))
    }

    async fn mark_task_failed(&self, _task_id: Uuid, _message: &str) -> Result<(), AppError> {
        Err(Self::unsupported("mark_task_failed"))
    }

    async fn insert_result_and_complete(&self, _result: AnalysisResult) -> Result<(), AppError> {
        Err(Self::unsupported("insert_result_and_complete"))
    }

    async fn insert_dead_letter(&self, _dead_letter: AnalysisDeadLetter) -> Result<(), AppError> {
        Err(Self::unsupported("insert_dead_letter"))
    }
}

fn storage_error(message: &'static str) -> impl Fn(sqlx::Error) -> AppError {
    move |source| AppError::Storage {
        message: message.to_string(),
        source: Some(Box::new(source)),
    }
}

fn row_decode_error(message: &'static str, source: sqlx::Error) -> AppError {
    AppError::Storage {
        message: message.to_string(),
        source: Some(Box::new(source)),
    }
}

fn parse_json_field(field: &'static str, value: String) -> Result<serde_json::Value, AppError> {
    serde_json::from_str(&value).map_err(|source| AppError::Storage {
        message: format!("failed to decode {field} json"),
        source: Some(Box::new(source)),
    })
}

fn map_task_row(row: sqlx::postgres::PgRow) -> Result<AnalysisTask, AppError> {
    let status =
        row.try_get::<String, _>("status")
            .map_err(|source| row_decode_error("failed to decode task status", source))?;
    let bar_state = row
        .try_get::<String, _>("bar_state")
        .map_err(|source| row_decode_error("failed to decode task bar_state", source))?;
    let timeframe =
        row.try_get::<Option<String>, _>("timeframe")
            .map_err(|source| row_decode_error("failed to decode task timeframe", source))?;
    let attempt_count = row
        .try_get::<i32, _>("attempt_count")
        .map_err(|source| row_decode_error("failed to decode task attempt_count", source))?;
    let max_attempts = row
        .try_get::<i32, _>("max_attempts")
        .map_err(|source| row_decode_error("failed to decode task max_attempts", source))?;

    Ok(AnalysisTask {
        id: row
            .try_get("id")
            .map_err(|source| row_decode_error("failed to decode task id", source))?,
        task_type: row
            .try_get("task_type")
            .map_err(|source| row_decode_error("failed to decode task_type", source))?,
        status: AnalysisTaskStatus::from_db(&status).ok_or_else(|| AppError::Storage {
            message: format!("invalid task status in storage: {status}"),
            source: None,
        })?,
        instrument_id: row
            .try_get("instrument_id")
            .map_err(|source| row_decode_error("failed to decode instrument_id", source))?,
        user_id: row
            .try_get("user_id")
            .map_err(|source| row_decode_error("failed to decode user_id", source))?,
        timeframe: timeframe.map(|value| value.parse()).transpose()?,
        bar_state: AnalysisBarState::from_db(&bar_state).ok_or_else(|| AppError::Storage {
            message: format!("invalid task bar_state in storage: {bar_state}"),
            source: None,
        })?,
        bar_open_time: row
            .try_get("bar_open_time")
            .map_err(|source| row_decode_error("failed to decode bar_open_time", source))?,
        bar_close_time: row
            .try_get("bar_close_time")
            .map_err(|source| row_decode_error("failed to decode bar_close_time", source))?,
        trading_date: row
            .try_get("trading_date")
            .map_err(|source| row_decode_error("failed to decode trading_date", source))?,
        trigger_type: row
            .try_get("trigger_type")
            .map_err(|source| row_decode_error("failed to decode trigger_type", source))?,
        prompt_key: row
            .try_get("prompt_key")
            .map_err(|source| row_decode_error("failed to decode prompt_key", source))?,
        prompt_version: row
            .try_get("prompt_version")
            .map_err(|source| row_decode_error("failed to decode prompt_version", source))?,
        snapshot_id: row
            .try_get("snapshot_id")
            .map_err(|source| row_decode_error("failed to decode snapshot_id", source))?,
        dedupe_key: row
            .try_get("dedupe_key")
            .map_err(|source| row_decode_error("failed to decode dedupe_key", source))?,
        attempt_count: u32::try_from(attempt_count).map_err(|source| AppError::Storage {
            message: "task attempt_count was negative in storage".to_string(),
            source: Some(Box::new(source)),
        })?,
        max_attempts: u32::try_from(max_attempts).map_err(|source| AppError::Storage {
            message: "task max_attempts was negative in storage".to_string(),
            source: Some(Box::new(source)),
        })?,
        scheduled_at: row
            .try_get("scheduled_at")
            .map_err(|source| row_decode_error("failed to decode scheduled_at", source))?,
        started_at: row
            .try_get("started_at")
            .map_err(|source| row_decode_error("failed to decode started_at", source))?,
        finished_at: row
            .try_get("finished_at")
            .map_err(|source| row_decode_error("failed to decode finished_at", source))?,
        last_error_code: row
            .try_get("last_error_code")
            .map_err(|source| row_decode_error("failed to decode last_error_code", source))?,
        last_error_message: row
            .try_get("last_error_message")
            .map_err(|source| row_decode_error("failed to decode last_error_message", source))?,
    })
}

fn map_snapshot_row(row: sqlx::postgres::PgRow) -> Result<AnalysisSnapshot, AppError> {
    Ok(AnalysisSnapshot {
        id: row
            .try_get("id")
            .map_err(|source| row_decode_error("failed to decode snapshot id", source))?,
        task_id: row
            .try_get("task_id")
            .map_err(|source| row_decode_error("failed to decode snapshot task_id", source))?,
        input_json: parse_json_field(
            "snapshot input_json",
            row.try_get("input_json_text")
                .map_err(|source| row_decode_error("failed to decode snapshot input_json", source))?,
        )?,
        input_hash: row
            .try_get("input_hash")
            .map_err(|source| row_decode_error("failed to decode snapshot input_hash", source))?,
        schema_version: row.try_get("schema_version").map_err(|source| {
            row_decode_error("failed to decode snapshot schema_version", source)
        })?,
        created_at: row
            .try_get("created_at")
            .map_err(|source| row_decode_error("failed to decode snapshot created_at", source))?,
    })
}

fn map_attempt_row(row: sqlx::postgres::PgRow) -> Result<AnalysisAttempt, AppError> {
    let attempt_no = row
        .try_get::<i32, _>("attempt_no")
        .map_err(|source| row_decode_error("failed to decode attempt_no", source))?;

    Ok(AnalysisAttempt {
        id: row
            .try_get("id")
            .map_err(|source| row_decode_error("failed to decode attempt id", source))?,
        task_id: row
            .try_get("task_id")
            .map_err(|source| row_decode_error("failed to decode attempt task_id", source))?,
        attempt_no: u32::try_from(attempt_no).map_err(|source| AppError::Storage {
            message: "attempt_no was negative in storage".to_string(),
            source: Some(Box::new(source)),
        })?,
        worker_id: row
            .try_get("worker_id")
            .map_err(|source| row_decode_error("failed to decode worker_id", source))?,
        llm_provider: row
            .try_get("llm_provider")
            .map_err(|source| row_decode_error("failed to decode llm_provider", source))?,
        model: row
            .try_get("model")
            .map_err(|source| row_decode_error("failed to decode model", source))?,
        request_payload_json: parse_json_field(
            "attempt request_payload_json",
            row.try_get("request_payload_json_text").map_err(|source| {
                row_decode_error("failed to decode request_payload_json", source)
            })?,
        )?,
        raw_response_json: row
            .try_get::<Option<String>, _>("raw_response_json_text")
            .map_err(|source| row_decode_error("failed to decode raw_response_json", source))?
            .map(|value| parse_json_field("attempt raw_response_json", value))
            .transpose()?,
        parsed_output_json: row
            .try_get::<Option<String>, _>("parsed_output_json_text")
            .map_err(|source| row_decode_error("failed to decode parsed_output_json", source))?
            .map(|value| parse_json_field("attempt parsed_output_json", value))
            .transpose()?,
        status: row
            .try_get("status")
            .map_err(|source| row_decode_error("failed to decode attempt status", source))?,
        error_type: row
            .try_get("error_type")
            .map_err(|source| row_decode_error("failed to decode error_type", source))?,
        error_message: row
            .try_get("error_message")
            .map_err(|source| row_decode_error("failed to decode error_message", source))?,
        started_at: row
            .try_get("started_at")
            .map_err(|source| row_decode_error("failed to decode started_at", source))?,
        finished_at: row
            .try_get("finished_at")
            .map_err(|source| row_decode_error("failed to decode finished_at", source))?,
    })
}

fn map_result_row(row: sqlx::postgres::PgRow) -> Result<AnalysisResult, AppError> {
    let timeframe =
        row.try_get::<Option<String>, _>("timeframe")
            .map_err(|source| row_decode_error("failed to decode result timeframe", source))?;
    let bar_state = row
        .try_get::<String, _>("bar_state")
        .map_err(|source| row_decode_error("failed to decode result bar_state", source))?;

    Ok(AnalysisResult {
        id: row
            .try_get("id")
            .map_err(|source| row_decode_error("failed to decode result id", source))?,
        task_id: row
            .try_get("task_id")
            .map_err(|source| row_decode_error("failed to decode result task_id", source))?,
        task_type: row
            .try_get("task_type")
            .map_err(|source| row_decode_error("failed to decode result task_type", source))?,
        instrument_id: row
            .try_get("instrument_id")
            .map_err(|source| row_decode_error("failed to decode result instrument_id", source))?,
        user_id: row
            .try_get("user_id")
            .map_err(|source| row_decode_error("failed to decode result user_id", source))?,
        timeframe: timeframe.map(|value| value.parse()).transpose()?,
        bar_state: AnalysisBarState::from_db(&bar_state).ok_or_else(|| AppError::Storage {
            message: format!("invalid result bar_state in storage: {bar_state}"),
            source: None,
        })?,
        bar_open_time: row
            .try_get("bar_open_time")
            .map_err(|source| row_decode_error("failed to decode result bar_open_time", source))?,
        bar_close_time: row.try_get("bar_close_time").map_err(|source| {
            row_decode_error("failed to decode result bar_close_time", source)
        })?,
        trading_date: row
            .try_get("trading_date")
            .map_err(|source| row_decode_error("failed to decode result trading_date", source))?,
        prompt_key: row
            .try_get("prompt_key")
            .map_err(|source| row_decode_error("failed to decode result prompt_key", source))?,
        prompt_version: row.try_get("prompt_version").map_err(|source| {
            row_decode_error("failed to decode result prompt_version", source)
        })?,
        output_json: parse_json_field(
            "result output_json",
            row.try_get("output_json_text")
                .map_err(|source| row_decode_error("failed to decode output_json", source))?,
        )?,
        created_at: row
            .try_get("created_at")
            .map_err(|source| row_decode_error("failed to decode result created_at", source))?,
    })
}

fn map_dead_letter_row(row: sqlx::postgres::PgRow) -> Result<AnalysisDeadLetter, AppError> {
    Ok(AnalysisDeadLetter {
        id: row
            .try_get("id")
            .map_err(|source| row_decode_error("failed to decode dead letter id", source))?,
        task_id: row
            .try_get("task_id")
            .map_err(|source| row_decode_error("failed to decode dead letter task_id", source))?,
        final_error_type: row.try_get("final_error_type").map_err(|source| {
            row_decode_error("failed to decode dead letter final_error_type", source)
        })?,
        final_error_message: row.try_get("final_error_message").map_err(|source| {
            row_decode_error("failed to decode dead letter final_error_message", source)
        })?,
        last_attempt_id: row.try_get("last_attempt_id").map_err(|source| {
            row_decode_error("failed to decode dead letter last_attempt_id", source)
        })?,
        archived_snapshot_json: parse_json_field(
            "dead letter archived_snapshot_json",
            row.try_get("archived_snapshot_json_text").map_err(|source| {
                row_decode_error("failed to decode archived_snapshot_json", source)
            })?,
        )?,
        created_at: row.try_get("created_at").map_err(|source| {
            row_decode_error("failed to decode dead letter created_at", source)
        })?,
    })
}
