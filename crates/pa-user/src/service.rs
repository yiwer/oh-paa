use crate::{
    models::{ManualUserAnalysisRequest, UserAnalysisReport},
    repository::{SharedAnalysisLookup, UserRepository},
};
use pa_core::AppError;

pub struct UserAnalysisService<'a, U, S>
where
    U: UserRepository + ?Sized,
    S: SharedAnalysisLookup + ?Sized,
{
    user_repository: &'a U,
    shared_analysis_lookup: &'a S,
}

impl<'a, U, S> UserAnalysisService<'a, U, S>
where
    U: UserRepository + ?Sized,
    S: SharedAnalysisLookup + ?Sized,
{
    pub fn new(user_repository: &'a U, shared_analysis_lookup: &'a S) -> Self {
        Self {
            user_repository,
            shared_analysis_lookup,
        }
    }

    pub async fn run_manual_analysis(
        &self,
        request: ManualUserAnalysisRequest,
    ) -> Result<UserAnalysisReport, AppError> {
        let subscriptions = self
            .user_repository
            .list_user_subscriptions(request.user_id)
            .await?;
        let positions = self
            .user_repository
            .list_user_positions(request.user_id, request.instrument_id)
            .await?;
        let bar_analysis = self
            .shared_analysis_lookup
            .get_bar_analysis(
                request.instrument_id,
                request.timeframe,
                request.bar_close_time,
                &request.analysis_version,
            )
            .await?
            .ok_or_else(|| AppError::Analysis {
                message: format!(
                    "missing shared bar analysis for instrument_id={}, timeframe={}, bar_close_time={}, analysis_version={}",
                    request.instrument_id,
                    request.timeframe.as_str(),
                    request.bar_close_time.to_rfc3339(),
                    request.analysis_version
                ),
                source: None,
            })?;
        let daily_market_context = self
            .shared_analysis_lookup
            .get_daily_market_context(
                request.instrument_id,
                request.trading_date,
                &request.analysis_version,
            )
            .await?
            .ok_or_else(|| AppError::Analysis {
                message: format!(
                    "missing shared daily market context for instrument_id={}, timeframe={}, trading_date={}, analysis_version={}",
                    request.instrument_id,
                    request.timeframe.as_str(),
                    request.trading_date,
                    request.analysis_version
                ),
                source: None,
            })?;

        Ok(UserAnalysisReport {
            user_id: request.user_id,
            instrument_id: request.instrument_id,
            subscriptions,
            positions,
            bar_analysis: bar_analysis.result_json,
            daily_market_context: daily_market_context.context_json,
        })
    }
}
