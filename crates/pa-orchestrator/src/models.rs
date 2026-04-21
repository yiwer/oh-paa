use serde_json::Value;

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
