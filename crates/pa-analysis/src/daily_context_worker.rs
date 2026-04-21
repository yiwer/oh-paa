use chrono::NaiveDate;
use pa_core::AppError;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    models::DailyMarketContext,
    repository::AnalysisRepository,
    service::{AnalysisService, GenerationResult},
};

#[derive(Debug, Clone, PartialEq)]
pub struct DailyContextTask {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub analysis_version: String,
    pub context_json: Value,
}

impl DailyContextTask {
    pub fn into_context(self) -> DailyMarketContext {
        DailyMarketContext {
            instrument_id: self.instrument_id,
            trading_date: self.trading_date,
            analysis_version: self.analysis_version,
            context_json: self.context_json,
        }
    }
}

pub async fn run_daily_context_task<R>(
    service: &AnalysisService<'_, R>,
    task: DailyContextTask,
) -> Result<GenerationResult<DailyMarketContext>, AppError>
where
    R: AnalysisRepository + ?Sized,
{
    service.generate_daily_context(task).await
}
