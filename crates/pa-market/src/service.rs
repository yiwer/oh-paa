use chrono::Utc;
use pa_core::{AppError, Timeframe};
use uuid::Uuid;

use crate::{
    normalize_kline,
    provider::ProviderRouter,
    repository::{CanonicalKlineRepository, CanonicalKlineRow},
};

#[derive(Debug, Clone)]
pub struct BackfillCanonicalKlinesRequest<'a> {
    pub instrument_id: Uuid,
    pub provider_symbol: &'a str,
    pub timeframe: Timeframe,
    pub limit: usize,
    pub primary_provider: &'a str,
    pub fallback_provider: &'a str,
}

pub async fn backfill_canonical_klines(
    router: &ProviderRouter,
    repository: &dyn CanonicalKlineRepository,
    request: BackfillCanonicalKlinesRequest<'_>,
) -> Result<(), AppError> {
    let routed = router
        .fetch_klines_with_fallback_source(
            request.primary_provider,
            request.fallback_provider,
            request.provider_symbol,
            request.timeframe,
            request.limit,
        )
        .await?;

    for bar in routed.klines {
        let normalized = normalize_kline(bar)?;
        if normalized.close_time > Utc::now() {
            continue;
        }

        repository
            .upsert_canonical_kline(CanonicalKlineRow {
                instrument_id: request.instrument_id,
                timeframe: request.timeframe,
                open_time: normalized.open_time,
                close_time: normalized.close_time,
                open: normalized.open,
                high: normalized.high,
                low: normalized.low,
                close: normalized.close,
                volume: normalized.volume,
                source_provider: routed.provider_name.to_string(),
            })
            .await?;
    }

    Ok(())
}
