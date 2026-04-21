use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    models::BarAnalysis,
    repository::AnalysisRepository,
    service::{AnalysisService, GenerationResult},
};

#[derive(Debug, Clone, PartialEq)]
pub struct BarAnalysisTask {
    pub instrument_id: Uuid,
    pub timeframe: Timeframe,
    pub bar_close_time: DateTime<Utc>,
    pub analysis_version: String,
    pub result_json: Value,
}

impl BarAnalysisTask {
    pub fn into_analysis(self) -> BarAnalysis {
        BarAnalysis {
            instrument_id: self.instrument_id,
            timeframe: self.timeframe,
            bar_close_time: self.bar_close_time,
            analysis_version: self.analysis_version,
            result_json: self.result_json,
        }
    }
}

pub async fn run_bar_analysis_task<R>(
    service: &AnalysisService<'_, R>,
    task: BarAnalysisTask,
) -> Result<GenerationResult<BarAnalysis>, AppError>
where
    R: AnalysisRepository + ?Sized,
{
    service.generate_bar_analysis(task).await
}
