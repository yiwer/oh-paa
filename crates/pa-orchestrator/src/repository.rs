use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, MutexGuard},
};

use async_trait::async_trait;
use pa_core::AppError;
use uuid::Uuid;

use crate::{AnalysisBarState, AnalysisSnapshot, AnalysisTask, AnalysisTaskStatus};

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

    async fn load_snapshot(&self, snapshot_id: Uuid) -> Result<AnalysisSnapshot, AppError>;
}

#[derive(Debug, Default)]
pub struct InMemoryOrchestrationRepository {
    state: Mutex<InMemoryState>,
}

#[derive(Debug, Default)]
struct InMemoryState {
    tasks: HashMap<Uuid, AnalysisTask>,
    snapshots: HashMap<Uuid, AnalysisSnapshot>,
    pending_order: VecDeque<Uuid>,
    closed_dedupe: HashMap<String, Uuid>,
}

impl InMemoryOrchestrationRepository {
    fn lock_state(&self) -> MutexGuard<'_, InMemoryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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

        if matches!(task.bar_state, AnalysisBarState::Closed)
            && let Some(dedupe_key) = task.dedupe_key.as_ref()
            && let Some(existing_task_id) = state.closed_dedupe.get(dedupe_key)
        {
            return Ok(InsertTaskResult::DuplicateExistingTask(*existing_task_id));
        }

        if matches!(task.bar_state, AnalysisBarState::Closed)
            && let Some(dedupe_key) = task.dedupe_key.as_ref()
        {
            state.closed_dedupe.insert(dedupe_key.clone(), task.id);
        }

        if matches!(task.status, AnalysisTaskStatus::Pending) {
            state.pending_order.push_back(task.id);
        }

        state.snapshots.insert(snapshot.id, snapshot);
        state.tasks.insert(task.id, task);

        Ok(InsertTaskResult::Inserted)
    }

    async fn fetch_next_pending_task(&self) -> Result<Option<AnalysisTask>, AppError> {
        let mut state = self.lock_state();
        while let Some(task_id) = state.pending_order.pop_front() {
            if let Some(task) = state.tasks.get(&task_id) {
                return Ok(Some(task.clone()));
            }
        }

        Ok(None)
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
}
