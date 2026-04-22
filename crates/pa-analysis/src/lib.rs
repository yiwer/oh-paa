pub mod bar_worker;
pub mod daily_context_worker;
pub mod models;
pub mod prompt_specs;
pub mod repository;
pub mod service;
pub mod task_factory;

pub use bar_worker::BarAnalysisTask;
pub use daily_context_worker::DailyContextTask;
pub use models::{
    BarAnalysis, DailyMarketContext, PaStateBar, SharedBarAnalysisInput, SharedBarAnalysisOutput,
    SharedDailyContextInput, SharedDailyContextOutput, SharedPaStateBarInput,
    SharedPaStateBarOutput,
};
pub use prompt_specs::{
    shared_bar_analysis_prompt_v2, shared_bar_analysis_v1, shared_bar_analysis_v2,
    shared_daily_context_prompt_v2, shared_daily_context_v1, shared_daily_context_v2,
    shared_pa_state_bar_prompt_v1, shared_pa_state_bar_v1,
};
pub use repository::{AnalysisRepository, InMemoryAnalysisRepository};
pub use service::{AnalysisService, GenerationResult};
pub use task_factory::{
    build_shared_bar_analysis_task, build_shared_daily_context_task, build_shared_pa_state_bar_task,
};
