use pa_core::AppError;

use crate::{
    bar_worker::BarAnalysisTask,
    daily_context_worker::DailyContextTask,
    models::{BarAnalysis, DailyMarketContext},
    repository::AnalysisRepository,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GenerationResult<T> {
    pub created: bool,
    pub record: T,
}

pub struct AnalysisService<'a, R>
where
    R: AnalysisRepository + ?Sized,
{
    repository: &'a R,
}

impl<'a, R> AnalysisService<'a, R>
where
    R: AnalysisRepository + ?Sized,
{
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub async fn generate_bar_analysis(
        &self,
        task: BarAnalysisTask,
    ) -> Result<GenerationResult<BarAnalysis>, AppError> {
        let analysis = task.into_analysis();
        let created = self
            .repository
            .insert_bar_analysis_if_absent(analysis.clone())
            .await?;

        Ok(GenerationResult {
            created,
            record: analysis,
        })
    }

    pub async fn generate_daily_context(
        &self,
        task: DailyContextTask,
    ) -> Result<GenerationResult<DailyMarketContext>, AppError> {
        let context = task.into_context();
        let created = self
            .repository
            .insert_daily_context_if_absent(context.clone())
            .await?;

        Ok(GenerationResult {
            created,
            record: context,
        })
    }
}
