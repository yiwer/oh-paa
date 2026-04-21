use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use chrono::Utc;
use pa_core::AppError;
use uuid::Uuid;

use crate::{
    AnalysisAttempt, AnalysisDeadLetter, AnalysisResult, AnalysisSnapshot, AnalysisTask,
    AnalysisTaskStatus,
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

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError>;

    async fn claim_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError>;

    async fn release_claimed_task(&self, task_id: Uuid, message: &str) -> Result<(), AppError>;

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError>;

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

    pub fn remove_snapshot(&self, snapshot_id: Uuid) {
        self.lock_state().snapshots.remove(&snapshot_id);
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
        let task = state.tasks.get_mut(&task_id).ok_or_else(|| AppError::Storage {
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

    async fn mark_task_running(&self, task_id: Uuid) -> Result<(), AppError> {
        let mut state = self.lock_state();
        let task = state.tasks.get_mut(&task_id).ok_or_else(|| AppError::Storage {
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
        let task = state.tasks.get_mut(&task_id).ok_or_else(|| AppError::Storage {
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
        let task = state.tasks.get_mut(&task_id).ok_or_else(|| AppError::Storage {
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
