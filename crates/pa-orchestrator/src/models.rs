use chrono::{DateTime, NaiveDate, Utc};
use pa_core::AppError;
use pa_core::Timeframe;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisTaskStatus {
    Pending,
    Running,
    RetryWaiting,
    Succeeded,
    Failed,
    DeadLetter,
    Cancelled,
}

impl AnalysisTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::RetryWaiting => "retry_waiting",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "retry_waiting" => Some(Self::RetryWaiting),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "dead_letter" => Some(Self::DeadLetter),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisBarState {
    None,
    Open,
    Closed,
}

impl AnalysisBarState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicyClass {
    NetworkTransient,
    LlmRateLimited,
    LlmStructuredOutput,
    DomainValidation,
}

impl RetryPolicyClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NetworkTransient => "network_transient",
            Self::LlmRateLimited => "llm_rate_limited",
            Self::LlmStructuredOutput => "llm_structured_output",
            Self::DomainValidation => "domain_validation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptResultSemantics {
    SharedAsset,
    UserPrivateAsset,
}

impl PromptResultSemantics {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SharedAsset => "shared_asset",
            Self::UserPrivateAsset => "user_private_asset",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptSpec {
    pub prompt_key: String,
    pub prompt_version: String,
    pub task_type: String,
    pub system_prompt: String,
    pub input_schema_version: String,
    pub output_schema_version: String,
    pub output_json_schema: Value,
    pub retry_policy_class: RetryPolicyClass,
    pub result_semantics: PromptResultSemantics,
    pub bar_state_support: Vec<AnalysisBarState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisTask {
    pub id: Uuid,
    pub task_type: String,
    pub status: AnalysisTaskStatus,
    pub instrument_id: Uuid,
    pub user_id: Option<Uuid>,
    pub timeframe: Option<Timeframe>,
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub trigger_type: String,
    pub prompt_key: String,
    pub prompt_version: String,
    pub snapshot_id: Uuid,
    pub dedupe_key: Option<String>,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSnapshot {
    pub id: Uuid,
    pub task_id: Uuid,
    pub input_json: Value,
    pub input_hash: String,
    pub schema_version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisAttempt {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt_no: u32,
    pub worker_id: String,
    pub llm_provider: String,
    pub model: String,
    pub request_payload_json: Value,
    pub raw_response_json: Option<Value>,
    pub parsed_output_json: Option<Value>,
    pub status: String,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    pub id: Uuid,
    pub task_id: Uuid,
    pub task_type: String,
    pub instrument_id: Uuid,
    pub user_id: Option<Uuid>,
    pub timeframe: Option<Timeframe>,
    pub bar_state: AnalysisBarState,
    pub bar_open_time: Option<DateTime<Utc>>,
    pub bar_close_time: Option<DateTime<Utc>>,
    pub trading_date: Option<NaiveDate>,
    pub prompt_key: String,
    pub prompt_version: String,
    pub output_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisDeadLetter {
    pub id: Uuid,
    pub task_id: Uuid,
    pub final_error_type: String,
    pub final_error_message: String,
    pub last_attempt_id: Option<Uuid>,
    pub archived_snapshot_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskEnvelope {
    pub task: AnalysisTask,
    pub snapshot: AnalysisSnapshot,
}

impl AnalysisResult {
    pub fn from_task(task: &AnalysisTask, output_json: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_id: task.id,
            task_type: task.task_type.clone(),
            instrument_id: task.instrument_id,
            user_id: task.user_id,
            timeframe: task.timeframe,
            bar_state: task.bar_state,
            bar_open_time: task.bar_open_time,
            bar_close_time: task.bar_close_time,
            trading_date: task.trading_date,
            prompt_key: task.prompt_key.clone(),
            prompt_version: task.prompt_version.clone(),
            output_json,
            created_at: Utc::now(),
        }
    }
}

impl AnalysisDeadLetter {
    pub fn from_task_and_error(
        task: &AnalysisTask,
        snapshot: &AnalysisSnapshot,
        err: &AppError,
        last_attempt_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_id: task.id,
            final_error_type: app_error_type(err).to_string(),
            final_error_message: err.to_string(),
            last_attempt_id,
            archived_snapshot_json: snapshot.input_json.clone(),
            created_at: Utc::now(),
        }
    }
}

fn app_error_type(err: &AppError) -> &'static str {
    match err {
        AppError::Validation { .. } => "validation",
        AppError::Provider { .. } => "provider",
        AppError::Storage { .. } => "storage",
        AppError::Analysis { .. } => "analysis",
    }
}
