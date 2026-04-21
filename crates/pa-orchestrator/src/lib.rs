mod dedupe;
mod executor;
mod llm;
mod models;
mod prompt_registry;
mod repository;

pub use dedupe::{build_shared_bar_dedupe_key, sha256_json};
pub use executor::Executor;
pub use llm::{FixtureLlmClient, LlmClient};
pub use models::{
    AnalysisAttempt, AnalysisBarState, AnalysisDeadLetter, AnalysisResult, AnalysisSnapshot,
    AnalysisTask, AnalysisTaskStatus, PromptResultSemantics, PromptSpec, RetryPolicyClass,
    TaskEnvelope,
};
pub use prompt_registry::PromptRegistry;
pub use repository::{InMemoryOrchestrationRepository, InsertTaskResult, OrchestrationRepository};
